use std::fs::{self, File};
use std::io::{stdout, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};
use clap::builder::ArgPredicate;
use clap::Args;
use futures::{stream, StreamExt};
use indexmap::{IndexMap, IndexSet};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use itertools::Itertools;
use pkgcraft::cli::{pkgs_ebuild, MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::dep::Uri;
use pkgcraft::error::Error;
use pkgcraft::macros::build_path;
use pkgcraft::pkg::ebuild::manifest::Manifest;
use pkgcraft::pkg::{Package, RepoPackage};
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::{Contains, LogErrors};
use pkgcraft::utils::bounded_jobs;
use rayon::prelude::*;
use reqwest::StatusCode;
use tempfile::TempDir;
use tracing::{error, warn};

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
            fs::create_dir_all(value).map_err(|e| anyhow!("failed creating directory: {e}"))?;
            Ok(Self::Path(value.to_path_buf()))
        } else {
            let tmpdir =
                TempDir::new().map_err(|e| anyhow!("failed creating temporary directory: {e}"))?;
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

#[derive(Debug, Args)]
#[clap(next_help_heading = "Target options")]
pub(crate) struct Command {
    /// Concurrent downloads
    #[arg(short, long, default_value = "3")]
    concurrent: usize,

    /// Destination directory
    #[arg(short, long)]
    dir: Option<Utf8PathBuf>,

    /// Force remanifest
    #[arg(short, long)]
    force: bool,

    /// Ignore invalid service certificates
    #[arg(short, long)]
    insecure: bool,

    /// Connection timeout in seconds
    #[arg(short, long, default_value = "15")]
    timeout: f64,

    /// Disable progress output
    #[arg(short, long)]
    no_progress: bool,

    /// Output to stdout
    #[arg(long)]
    stdout: bool,

    /// Target repo
    #[arg(long)]
    repo: Option<String>,

    /// Process fetch-restricted packages
    #[arg(long)]
    restrict: bool,

    /// Force thick manifests
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    thick: Option<bool>,

    // positionals
    /// Target packages or paths
    #[arg(
        // default to the current working directory
        default_value = ".",
        // default to all packages when targeting a repo
        default_value_if("repo", ArgPredicate::IsPresent, Some("*")),
        help_heading = "Arguments",
    )]
    targets: Vec<MaybeStdinVec<String>>,
}

// TODO: support custom templates or colors?
/// Create a progress bar for a file download.
fn progress_bar(hidden: bool) -> ProgressBar {
    let pb = if hidden {
        ProgressBar::hidden()
    } else {
        ProgressBar::no_length()
    };
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
        .progress_chars("#>-"));
    pb
}

// TODO: move to async closure once they are stabilized
/// Download the file related to a URI.
async fn download(
    client: &reqwest::Client,
    uri: &Uri,
    path: &Utf8Path,
    pb: &ProgressBar,
    mut size: Option<u64>,
) -> pkgcraft::Result<()> {
    // determine the file position to start at supporting resumed downloads
    let mut request = client.get(uri.as_ref());
    let mut position = if let Ok(meta) = fs::metadata(path) {
        // determine the target size for existing files without manifest entries
        if size.is_none() {
            let response = client.get(uri.as_ref()).send().await;
            size = response.ok().and_then(|r| r.content_length());
        }

        // check if completed or invalid
        let current_size = meta.len();
        if current_size - size.unwrap_or_default() == 0 {
            return Ok(());
        } else if let Some(value) = size {
            if current_size > value {
                return Err(Error::InvalidValue(format!("file larger than expected: {path}")));
            }
        }

        // request remaining data assuming sequential downloads
        request = request.header("Range", format!("bytes={current_size}-"));
        current_size
    } else {
        0
    };

    let response = request
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| Error::InvalidValue(format!("failed to get: {uri}: {e}")))?;

    // create file or open it for appending
    let mut file = match response.status() {
        StatusCode::PARTIAL_CONTENT => fs::OpenOptions::new().append(true).open(path),
        _ => File::create(path),
    }?;

    // initialize progress bar
    pb.set_message(format!("Downloading {uri}"));
    // enable completion progress if content size is available
    if let Some(value) = size.or(response.content_length()) {
        pb.set_length(value);
    }
    pb.set_position(position);
    // reset progress bar state so resumed download speed is accurate
    pb.reset();

    // download chunks while tracking progress
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk =
            item.map_err(|e| Error::InvalidValue(format!("error while downloading file: {e}")))?;
        file.write_all(&chunk)?;
        position += chunk.len() as u64;
        // TODO: handle progress differently for unsized downloads?
        pb.set_position(position);
    }

    pb.finish_and_clear();
    Ok(())
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let concurrent = bounded_jobs(self.concurrent);
        // TODO: pull DISTDIR from config for the default
        let dir = PathOrTempdir::new(self.dir.as_deref())?;
        let dir = dir.as_path()?;

        // convert targets to restrictions
        let targets: Vec<_> = TargetRestrictions::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .targets(self.targets.iter().flatten())
            .try_collect()?;
        config.finalize()?;

        // convert restrictions to pkgs
        let mut iter = pkgs_ebuild(targets).log_errors();

        let mut pkgs: IndexMap<_, IndexSet<_>> = IndexMap::new();
        for pkg in &mut iter {
            // determine if a manifest has an outdated or nonexistent entry
            let regen = |name: &str| -> bool {
                if let Some(entry) = pkg.manifest().get(name) {
                    // verify entry hashes match repo settings
                    entry
                        .hashes()
                        .keys()
                        .ne(&pkg.repo().metadata().config.manifest_hashes)
                } else {
                    true
                }
            };

            if self.restrict || !pkg.restrict().contains("fetch") {
                // TODO: flag or log unfetchable URIs
                let mut uris = pkg
                    .fetchables()
                    .filter(|uri| self.force || regen(uri.filename()))
                    .map(|uri| uri.into_owned())
                    .peekable();
                if uris.peek().is_some() {
                    pkgs.entry((pkg.repo(), pkg.cpn().clone()))
                        .or_default()
                        .extend(uris);
                }
            } else {
                warn!("skipping fetch restricted package: {pkg}");
            }
        }

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.insecure)
            .hickory_dns(true)
            .read_timeout(Duration::from_secs_f64(self.timeout))
            .connect_timeout(Duration::from_secs_f64(self.timeout))
            .referer(false)
            .build()
            .map_err(|e| anyhow!("failed creating client: {e}"))?;

        // show a global progress bar when downloading more files than concurrency limit
        let downloads = pkgs.values().flatten().count();
        let global_pb = if downloads > concurrent {
            Some(ProgressBar::new(downloads as u64))
        } else {
            None
        };

        // initialize progress handling
        let mb = MultiProgress::new();
        let hidden = !stdout().is_terminal() || self.no_progress;
        if hidden {
            mb.set_draw_target(ProgressDrawTarget::hidden());
        } else if let Some(pb) = global_pb.as_ref() {
            mb.add(pb.clone());
        }

        // download files asynchronously tracking failure status
        let failed = AtomicBool::new(false);
        tokio().block_on(async {
            // assume existing files are completely downloaded
            let targets = pkgs.iter().flat_map(|((repo, cpn), uris)| {
                uris.iter()
                    .filter(|uri| !dir.join(uri.filename()).exists())
                    .map(move |uri| {
                        let pkg_manifest = repo.metadata().manifest(cpn);
                        let manifest = pkg_manifest.get(uri.filename());
                        (uri, manifest.cloned())
                    })
            });

            // convert targets into download results stream
            let results = stream::iter(targets)
                .map(|(uri, manifest)| {
                    let client = &client;
                    let mb = &mb;
                    async move {
                        let pb = mb.add(progress_bar(hidden));
                        let size = manifest.as_ref().map(|m| m.size());
                        let path = dir.join(uri.filename());
                        let result = download(client, uri, &path, &pb, size).await;
                        mb.remove(&pb);
                        result
                    }
                })
                .buffer_unordered(concurrent);

            // process results stream while logging errors
            results
                .for_each(|result| async {
                    if let Err(e) = result {
                        mb.suspend(|| error!("{e}"));
                        failed.store(true, Ordering::Relaxed);
                    }

                    if let Some(pb) = global_pb.as_ref() {
                        pb.inc(1);
                    }
                })
                .await;
        });

        if let Some(pb) = global_pb.as_ref() {
            pb.finish_and_clear();
        }
        let mut stdout = stdout().lock();

        // create manifests if no download failures occurred
        if !failed.load(Ordering::Relaxed) {
            for ((repo, cpn), uris) in pkgs {
                let pkgdir = build_path!(&repo, cpn.category(), cpn.package());
                let manifest_path = pkgdir.join("Manifest");

                // load manifest from file
                let mut manifest = match Manifest::from_path(&manifest_path) {
                    Ok(value) => value,
                    Err(e) => {
                        error!("{e}");
                        Default::default()
                    }
                };

                // collect files for hashing
                let distfiles = uris.into_par_iter().map(|x| dir.join(x.filename()));

                // update manifest entries
                let thick = self
                    .thick
                    .unwrap_or_else(|| !repo.metadata().config.thin_manifests);
                let hashes = &repo.metadata().config.manifest_hashes;
                if let Err(e) = manifest.update(distfiles, hashes, &pkgdir, thick) {
                    error!("{e}");
                    failed.store(true, Ordering::Relaxed);
                    continue;
                }

                // write manifest to stdout or its file
                if self.stdout {
                    write!(&mut stdout, "{manifest}")?;
                } else {
                    fs::write(&manifest_path, format!("{manifest}"))?;
                };
            }
        }

        let status = iter.failed() | failed.load(Ordering::Relaxed);
        Ok(ExitCode::from(status as u8))
    }
}
