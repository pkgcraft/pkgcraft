use std::fs;
use std::io::{stdout, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use camino::Utf8PathBuf;
use clap::builder::ArgPredicate;
use clap::Args;
use futures::{stream, StreamExt};
use indexmap::IndexSet;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget};
use itertools::Itertools;
use pkgcraft::cli::{pkgs_ebuild, MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::error::Error;
use pkgcraft::fetch::Fetcher;
use pkgcraft::repo::RepoFormat;
use pkgcraft::restrict::{str::Restrict as StrRestrict, Restrict, Restriction};
use pkgcraft::traits::LogErrors;
use pkgcraft::utils::bounded_jobs;
use tracing::error;

use super::tokio;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Target options")]
pub(crate) struct Command {
    /// Concurrent downloads
    #[arg(short, long, default_value = "3")]
    concurrent: usize,

    /// Destination directory
    #[arg(short, long, default_value = ".")]
    dir: Utf8PathBuf,

    /// Filter URLs via regex
    #[arg(short = 'F', long, value_name = "REGEX")]
    filter: Option<String>,

    /// Overwrite existing files
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

    /// Output targets instead of fetching them
    #[arg(short, long)]
    pretend: bool,

    /// Connection timeout in seconds
    #[arg(short, long, default_value = "15")]
    timeout: f64,

    /// Target repo
    #[arg(long)]
    repo: Option<String>,

    /// Process fetch-restricted URLS
    #[arg(long)]
    restrict: bool,

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

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let concurrent = bounded_jobs(self.concurrent);
        let restrict = if let Some(value) = self.filter.as_deref() {
            StrRestrict::regex(value)?.into()
        } else {
            Restrict::True
        };
        // TODO: pull DISTDIR from config for the default
        fs::create_dir_all(&self.dir)?;

        // convert targets to restrictions
        let targets: Vec<_> = TargetRestrictions::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .targets(self.targets.iter().flatten())
            .try_collect()?;
        config.finalize()?;

        // convert restrictions to pkgs
        let mut iter = pkgs_ebuild(targets).log_errors();

        let failed = &AtomicBool::new(false);
        let mut fetchables = IndexSet::new();
        for pkg in &mut iter {
            fetchables.extend(
                pkg.fetchables(self.restrict, self.mirrors)
                    .filter_map(|result| match result {
                        Ok(value) => Some(value),
                        Err(e) => {
                            error!("{e}");
                            failed.store(true, Ordering::Relaxed);
                            None
                        }
                    })
                    .filter(|f| restrict.matches(f.as_str()))
                    .filter_map(|f| {
                        let path = self.dir.join(f.filename());
                        if self.force || !path.exists() {
                            let manifest = pkg.manifest().get(f.filename());
                            Some((f, path, manifest.cloned()))
                        } else {
                            None
                        }
                    }),
            );
        }

        // output targets if pretending to fetch
        if self.pretend {
            let mut stdout = stdout().lock();
            for (f, _, _) in fetchables.drain(..) {
                writeln!(stdout, "{f}")?;
            }
        }

        let builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.insecure)
            .hickory_dns(true)
            .read_timeout(Duration::from_secs_f64(self.timeout))
            .connect_timeout(Duration::from_secs_f64(self.timeout))
            .referer(false);
        let fetcher = &Fetcher::new(builder)?;

        // TODO: track overall download size if all targets have manifest data
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
            // convert fetchables into download results stream
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
                    if let Some(manifest) = manifest.as_ref() {
                        if result.is_ok() {
                            result = match tokio::fs::read(&src).await {
                                Ok(data) => manifest.verify(&data),
                                Err(e) => {
                                    Err(Error::InvalidValue(format!("failed reading: {src}: {e}")))
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

        let status = iter.failed() | failed.load(Ordering::Relaxed);
        Ok(ExitCode::from(status as u8))
    }
}
