use std::fs::{self, File};
use std::io::Write;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use clap::builder::ArgPredicate;
use clap::Args;
use crossbeam_channel::{bounded, Receiver};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::Itertools;
use pkgcraft::cli::{pkgs_ebuild, MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::dep::Uri;
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::LogErrors;
use pkgcraft::utils::bounded_jobs;
use tracing::error;

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

    /// Ignore invalid service certificates
    #[arg(short, long)]
    insecure: bool,

    /// Connection timeout in seconds
    #[arg(short, long, default_value = "15")]
    timeout: f64,

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
fn progress_bar() -> ProgressBar {
    let pb = ProgressBar::no_length();
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
        .progress_chars("#>-"));
    pb
}

/// Download the file related to a URI.
fn download_file(
    client: &reqwest::Client,
    uri: &Uri,
    path: &Utf8Path,
    pb: &ProgressBar,
) -> anyhow::Result<()> {
    tokio().block_on(async {
        let url = uri.uri();
        let res = client
            .get(url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| anyhow::anyhow!("failed to get: {url}: {e}"))?;

        // set up progress indicator
        pb.set_message(format!("Downloading {}", url));

        // enable completion progress if content size is available
        let total_size = res.content_length();
        if let Some(size) = &total_size {
            pb.set_length(*size);
        }

        // download chunks while tracking progress
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
    })
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let concurrent = bounded_jobs(self.concurrent);
        fs::create_dir_all(&self.dir)?;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.insecure)
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
        let fetch_failed = AtomicBool::new(false);

        thread::scope(|s| {
            let (uri_tx, uri_rx) = bounded(concurrent);
            let mb = MultiProgress::new();
            let failed = &fetch_failed;

            // create worker threads
            for _ in 0..concurrent {
                let client = client.clone();
                let uri_rx: Receiver<(Uri, Utf8PathBuf)> = uri_rx.clone();
                let mb = mb.clone();
                s.spawn(move || {
                    // TODO: skip non-http(s) URIs
                    for (uri, path) in uri_rx {
                        let pb = mb.add(progress_bar());
                        // TODO: add better error handling output
                        if let Err(e) = download_file(&client, &uri, &path, &pb) {
                            mb.suspend(|| {
                                error!("{e}");
                                failed.store(true, Ordering::Relaxed);
                            });
                        };
                        mb.remove(&pb);
                    }
                });
            }

            // send URIs to workers
            let mut iter = &mut iter;
            s.spawn(move || {
                for pkg in &mut iter {
                    for uri in pkg.src_uri().iter_flatten() {
                        let path = self.dir.join(uri.filename());
                        if !path.exists() {
                            uri_tx.send((uri.clone(), path)).ok();
                        }
                    }
                }
            });
        });

        let status = iter.failed() | fetch_failed.load(Ordering::Relaxed);
        Ok(ExitCode::from(status as u8))
    }
}
