use std::fs;
use std::io::{IsTerminal, Write, stdout};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};
use clap::{Args, builder::ArgPredicate};
use futures::{StreamExt, stream};
use indexmap::{IndexMap, IndexSet};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use pkgcraft::cli::{MaybeStdinVec, Targets};
use pkgcraft::config::Config;
use pkgcraft::error::Error;
use pkgcraft::fetch::{Fetchable, Fetcher};
use pkgcraft::macros::build_path;
use pkgcraft::repo::RepoFormat;
use pkgcraft::restrict::Scope;
use pkgcraft::utils::bounded_jobs;
use tempfile::TempDir;
use tracing::error;

use super::tokio;

/// Use a specific path or a new temporary directory.
enum PathOrTempdir {
    Path(Utf8PathBuf),
    Tempdir(TempDir),
}

impl PathOrTempdir {
    /// Create a new [`PathOrTempdir`] from an optional path.
    fn new(path: Option<&Utf8Path>) -> anyhow::Result<Self> {
        if let Some(value) = path {
            fs::create_dir_all(value)
                .map_err(|e| anyhow!("failed creating directory: {e}"))?;
            Ok(Self::Path(value.to_path_buf()))
        } else {
            let tmpdir = TempDir::new()
                .map_err(|e| anyhow!("failed creating temporary directory: {e}"))?;
            Ok(Self::Tempdir(tmpdir))
        }
    }

    /// Get the [`Utf8Path`] of the chosen location if possible.
    fn as_path(&self) -> anyhow::Result<&Utf8Path> {
        match self {
            Self::Path(path) => Ok(path),
            Self::Tempdir(tmpdir) => Utf8Path::from_path(tmpdir.path())
                .ok_or_else(|| anyhow!("invalid temporary directory")),
        }
    }
}

#[derive(Args)]
#[clap(next_help_heading = "Target options")]
pub(crate) struct Command {
    /// Concurrent downloads
    #[arg(short, long, default_value = "3")]
    concurrent: usize,

    /// Download directory
    #[arg(short, long)]
    dir: Option<Utf8PathBuf>,

    /// Force remanifest
    #[arg(short, long)]
    force: bool,

    /// Ignore invalid service certificates
    #[arg(short, long)]
    insecure: bool,

    /// Try fetching from default mirrors
    #[arg(short, long)]
    mirrors: bool,

    /// Disable progress output
    #[arg(short, long)]
    no_progress: bool,

    /// Connection timeout in seconds
    #[arg(short, long, default_value = "15")]
    timeout: f64,

    /// Output to stdout
    #[arg(long)]
    stdout: bool,

    /// Target repo
    #[arg(short, long)]
    repo: Option<String>,

    /// Process fetch-restricted URLS
    #[arg(long)]
    restrict: bool,

    /// Force manifest type
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        hide_possible_values = true,
        value_name = "BOOL",
    )]
    thick: Option<bool>,

    // positionals
    /// Target packages or paths
    #[arg(
        value_name = "TARGET",
        // default to the current working directory
        default_value = ".",
        // default to all packages when targeting a repo
        default_value_if("repo", ArgPredicate::IsPresent, Some("*")),
        help_heading = "Arguments",
    )]
    targets: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let concurrent = bounded_jobs(self.concurrent);
        // TODO: pull DISTDIR from config for the default
        let dir = PathOrTempdir::new(self.dir.as_deref())?;
        let dir = dir.as_path()?;

        // convert targets to pkg sets
        let pkg_sets = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .scope(|x| *x >= Scope::Package)
            .pkg_targets(self.targets.iter().flatten())?
            .collapse()
            .ebuild_pkg_sets()?;

        let failed = &AtomicBool::new(false);
        let mut fetchables = IndexSet::new();
        let mut pkg_distfiles = IndexMap::<_, IndexMap<_, _>>::new();
        for ((repo, cpn), pkgs) in pkg_sets {
            let manifest = repo.metadata().pkg_manifest(&cpn);

            // A manifest entry is regenerated if the entry hashes don't match the repo
            // hashes or the related file isn't in the manifest.
            let regen_entry = |name: &str| -> bool {
                if let Some(entry) = manifest.get(name) {
                    entry
                        .hashes()
                        .keys()
                        .ne(&repo.metadata().config.manifest_hashes)
                } else {
                    true
                }
            };

            for pkg in &pkgs {
                fetchables.extend(
                    pkg.src_uri()
                        .iter_flatten()
                        .filter_map(|uri| match Fetchable::from_uri(uri, pkg, self.mirrors) {
                            Ok(f) => Some(f),
                            Err(Error::RestrictedFetchable(f)) => {
                                let name = f.filename();
                                if self.restrict {
                                    Some(*f)
                                } else {
                                    if manifest.get(name).is_none() && !dir.join(name).exists()
                                    {
                                        error!("{pkg}: nonexistent restricted fetchable: {f}");
                                        failed.store(true, Ordering::Relaxed);
                                    }
                                    None
                                }
                            }
                            Err(Error::RestrictedFile(uri)) => {
                                let name = uri.filename();
                                if manifest.get(name).is_none() && !dir.join(name).exists() {
                                    error!("{pkg}: nonexistent restricted file: {name}");
                                    failed.store(true, Ordering::Relaxed);
                                }
                                None
                            }
                            Err(e) => {
                                error!("{pkg}: {e}");
                                failed.store(true, Ordering::Relaxed);
                                None
                            }
                        })
                        .filter(|f| self.force || regen_entry(f.filename()))
                        .filter_map(|f| {
                            let path = dir.join(f.filename());
                            // assume existing files are completely downloaded
                            if !path.exists() {
                                let manifest_entry = manifest.get(f.filename()).cloned();
                                Some((f, path, manifest_entry))
                            } else {
                                None
                            }
                        }),
                );
            }

            let thick = self
                .thick
                .unwrap_or_else(|| !repo.metadata().config.thin_manifests);
            let distfiles: IndexMap<_, _> = pkgs
                .iter()
                .flat_map(|x| x.distfiles())
                .map(|f| (f.to_string(), (dir.join(f), self.force || regen_entry(f))))
                .collect();
            let pkgdir = build_path!(&repo, cpn.category(), cpn.package());
            if self.force || manifest.outdated(&pkgdir, &distfiles, thick) {
                pkg_distfiles.insert((repo, cpn, pkgdir, thick), distfiles);
            }
        }

        let builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.insecure)
            .hickory_dns(true)
            .read_timeout(Duration::from_secs_f64(self.timeout))
            .connect_timeout(Duration::from_secs_f64(self.timeout))
            .referer(false);
        let fetcher = &Fetcher::new(builder)?;

        // show a global progress bar when downloading more files than concurrency limit
        let global_pb = if fetchables.len() > concurrent {
            Some(ProgressBar::new(fetchables.len() as u64))
        } else {
            None
        };

        // initialize progress handling
        let mb = &MultiProgress::new();
        let hidden = !stdout().is_terminal() || self.no_progress;
        if hidden {
            mb.set_draw_target(ProgressDrawTarget::hidden());
        } else if let Some(pb) = global_pb.as_ref() {
            mb.add(pb.clone());
        }

        // download files asynchronously tracking failure status
        let global_pb = &global_pb;
        tokio().block_on(async {
            // convert targets into download results stream
            let results = stream::iter(fetchables)
                .map(|(f, path, manifest)| async move {
                    let size = manifest.as_ref().map(|m| m.size());
                    let part_path = Utf8PathBuf::from(format!("{path}.part"));
                    let result = fetcher.fetch(&f, &part_path, mb, size).await;
                    (result, manifest, part_path, path)
                })
                .buffer_unordered(concurrent);

            // process results stream while logging errors
            results
                .for_each(|(mut result, manifest, src, dest)| async move {
                    // verify file hashes if manifest entry exists
                    if !self.force {
                        if let Some(manifest) = manifest.as_ref() {
                            if result.is_ok() {
                                result = match tokio::fs::read(&src).await {
                                    Ok(data) => manifest.verify(&data),
                                    Err(e) => Err(Error::InvalidValue(format!(
                                        "failed reading: {src}: {e}"
                                    ))),
                                }
                            }
                        }
                    }

                    if let Err(e) = result {
                        mb.suspend(|| error!("{e}"));
                        failed.store(true, Ordering::Relaxed);
                        fs::rename(src, format!("{dest}.failed")).ok();
                    } else {
                        fs::rename(src, dest).ok();
                    }

                    if let Some(pb) = global_pb.as_ref() {
                        pb.inc(1);
                    }
                })
                .await;
        });

        // clear global progress bar
        if let Some(pb) = global_pb.as_ref() {
            pb.finish_and_clear();
        }

        // create manifests if no download failures occurred
        if !failed.load(Ordering::Relaxed) {
            for ((repo, cpn, pkgdir, thick), distfiles) in pkg_distfiles {
                // load manifest from file
                let mut manifest = match repo.metadata().pkg_manifest_parse(&cpn) {
                    Ok(value) => value,
                    Err(e) => {
                        error!("{e}");
                        Default::default()
                    }
                };

                // update manifest entries
                let hashes = &repo.metadata().config.manifest_hashes;
                if let Err(e) = manifest.update(&distfiles, hashes, &pkgdir, thick) {
                    error!("{e}");
                    failed.store(true, Ordering::Relaxed);
                    continue;
                }

                // write manifest to target output
                let manifest_path = pkgdir.join("Manifest");
                if self.stdout {
                    write!(stdout(), "{manifest}")?;
                } else if !manifest.is_empty() {
                    fs::write(&manifest_path, manifest.to_string())
                        .map_err(|e| anyhow!("{cpn}::{repo}: failed writing manifest: {e}"))?;
                } else if manifest_path.exists() {
                    fs::remove_file(&manifest_path).map_err(|e| {
                        anyhow!("{cpn}::{repo}: failed removing manifest: {e}")
                    })?;
                }
            }
        }

        let status = failed.load(Ordering::Relaxed);
        Ok(ExitCode::from(status as u8))
    }
}
