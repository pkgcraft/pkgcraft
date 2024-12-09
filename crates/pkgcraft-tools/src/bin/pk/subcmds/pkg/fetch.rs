use std::fs::{self, File};
use std::io::{stdout, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use clap::builder::ArgPredicate;
use clap::Args;
use futures::{stream, StreamExt};
use indexmap::IndexSet;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use itertools::Itertools;
use pkgcraft::cli::{pkgs_ebuild, MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::dep::Uri;
use pkgcraft::repo::RepoFormat;
use pkgcraft::restrict::{str::Restrict as StrRestrict, Restrict, Restriction};
use pkgcraft::traits::{Contains, LogErrors};
use pkgcraft::utils::bounded_jobs;
use tracing::{error, warn};

#[derive(Debug, Args)]
#[clap(next_help_heading = "Target options")]
pub(crate) struct Command {
    /// Target repo
    #[arg(long)]
    repo: Option<String>,

    /// Concurrent downloads
    #[arg(short, long, default_value = "3")]
    concurrent: usize,

    /// Destination directory
    #[arg(short, long, default_value = ".")]
    dir: Utf8PathBuf,

    /// Filter URLs via regex
    #[arg(short, long, value_name = "REGEX")]
    filter: Option<String>,

    /// Ignore invalid service certificates
    #[arg(short, long)]
    insecure: bool,

    /// Connection timeout in seconds
    #[arg(short, long, default_value = "15")]
    timeout: f64,

    /// Disable progress output
    #[arg(short, long)]
    no_progress: bool,

    /// Process fetch-restricted packages
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

/// Return a static tokio runtime.
pub(crate) fn tokio() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
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
    uri: Uri,
    dir: &Utf8Path,
    pb: &ProgressBar,
) -> anyhow::Result<()> {
    let res = client
        .get(uri.as_ref())
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| anyhow::anyhow!("failed to get: {uri}: {e}"))?;

    // initialize progress header
    pb.set_message(format!("Downloading {uri}"));

    // enable completion progress if content size is available
    let total_size = res.content_length();
    if let Some(size) = &total_size {
        pb.set_length(*size);
    }

    // download chunks while tracking progress
    let path = dir.join(uri.filename());
    let mut file = File::create(path)?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| anyhow::anyhow!("error while downloading file: {e}"))?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        // TODO: handle progress differently for unsized downloads?
        pb.set_position(downloaded);
    }

    Ok(())
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let concurrent = bounded_jobs(self.concurrent);
        let restrict = if let Some(value) = self.filter.as_deref() {
            StrRestrict::regex(value)?.into()
        } else {
            Restrict::True
        };
        fs::create_dir_all(&self.dir)?;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.insecure)
            .hickory_dns(true)
            .read_timeout(Duration::from_secs_f64(self.timeout))
            .connect_timeout(Duration::from_secs_f64(self.timeout))
            .build()
            .map_err(|e| anyhow::anyhow!("failed creating client: {e}"))?;

        // convert targets to restrictions
        let targets: Vec<_> = TargetRestrictions::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .targets(self.targets.iter().flatten())
            .try_collect()?;
        config.finalize()?;

        // convert restrictions to pkgs
        let mut iter = pkgs_ebuild(targets).log_errors();

        // TODO: try pulling the file size from the pkg manifest if it exists
        let mut uris = IndexSet::new();
        for pkg in &mut iter {
            if self.restrict || !pkg.restrict().contains("fetch") {
                uris.extend(
                    pkg.src_uri()
                        .iter_flatten()
                        .filter(|u| restrict.matches(u.as_ref()))
                        .cloned(),
                );
            } else {
                warn!("skipping fetch restricted package: {pkg}");
            }
        }

        // TODO: track overall download size if all target URIs have manifest data
        // show a global progress bar when downloading more files than concurrency limit
        let global_pb = if uris.len() > concurrent {
            Some(ProgressBar::new(uris.len() as u64))
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

        let fetch_failed = AtomicBool::new(false);
        tokio().block_on(async {
            let results = stream::iter(uris)
                .map(|uri| {
                    let client = &client;
                    let mb = &mb;
                    async move {
                        let pb = mb.add(progress_bar(hidden));
                        let result = download(client, uri, &self.dir, &pb).await;
                        mb.remove(&pb);
                        result
                    }
                })
                .buffer_unordered(concurrent);

            results
                .for_each(|result| async {
                    if let Err(e) = result {
                        mb.suspend(|| {
                            error!("{e}");
                            fetch_failed.store(true, Ordering::Relaxed);
                        });
                    }
                    if let Some(pb) = global_pb.as_ref() {
                        pb.inc(1);
                    }
                })
                .await;
        });

        let status = iter.failed() | fetch_failed.load(Ordering::Relaxed);
        Ok(ExitCode::from(status as u8))
    }
}
