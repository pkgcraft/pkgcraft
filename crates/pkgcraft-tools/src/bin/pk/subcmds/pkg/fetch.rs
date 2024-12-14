use std::fs::{self, File};
use std::io::{stdout, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
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
use pkgcraft::error::Error;
use pkgcraft::repo::RepoFormat;
use pkgcraft::restrict::{str::Restrict as StrRestrict, Restrict, Restriction};
use pkgcraft::traits::{Contains, LogErrors};
use pkgcraft::utils::bounded_jobs;
use reqwest::StatusCode;
use tracing::{error, warn};

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

    /// Connection timeout in seconds
    #[arg(short, long, default_value = "15")]
    timeout: f64,

    /// Disable progress output
    #[arg(short, long)]
    no_progress: bool,

    /// Target repo
    #[arg(long)]
    repo: Option<String>,

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

        let mut uris = IndexSet::new();
        for pkg in &mut iter {
            if self.restrict || !pkg.restrict().contains("fetch") {
                uris.extend(
                    // TODO: flag or log unfetchable URIs
                    pkg.fetchables()
                        .filter(|uri| restrict.matches(uri.as_str()))
                        .filter_map(|uri| {
                            let path = self.dir.join(uri.filename());
                            if self.force || !path.exists() {
                                let manifest = pkg.manifest().get(uri.filename());
                                Some((uri.into_owned(), path, manifest.cloned()))
                            } else {
                                None
                            }
                        }),
                );
            } else {
                warn!("skipping fetch restricted package: {pkg}");
            }
        }

        let client = &reqwest::Client::builder()
            .danger_accept_invalid_certs(self.insecure)
            .hickory_dns(true)
            .read_timeout(Duration::from_secs_f64(self.timeout))
            .connect_timeout(Duration::from_secs_f64(self.timeout))
            .referer(false)
            .build()
            .map_err(|e| anyhow::anyhow!("failed creating client: {e}"))?;

        // TODO: track overall download size if all target URIs have manifest data
        // show a global progress bar when downloading more files than concurrency limit
        let global_pb = if uris.len() > concurrent {
            Some(ProgressBar::new(uris.len() as u64))
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
        let failed = &AtomicBool::new(false);
        let global_pb = &global_pb;
        tokio().block_on(async {
            // convert URIs into download results stream
            let results = stream::iter(uris)
                .map(|(uri, path, manifest)| async move {
                    let pb = mb.add(progress_bar(hidden));
                    let size = manifest.as_ref().map(|m| m.size());
                    let part_path = Utf8PathBuf::from(format!("{path}.part"));
                    let result = download(client, &uri, &part_path, &pb, size).await;
                    mb.remove(&pb);
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
