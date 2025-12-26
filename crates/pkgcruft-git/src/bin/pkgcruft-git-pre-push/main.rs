use std::io::{self, BufRead, IsTerminal};
use std::process::ExitCode;

use anyhow::anyhow;
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::colorize;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcraft::repo::RepoFormat;
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::current_dir;
use pkgcruft::report::ReportLevel;
use pkgcruft::reporter::{FancyReporter, Reporter};
use pkgcruft::scan::Scanner;
use pkgcruft_git::git;
use tracing_log::AsTrace;

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    version,
    long_about = None,
    disable_help_subcommand = true,
)]
/// pkgcruft-git client
pub(crate) struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

    #[command(flatten)]
    color: colorchoice_clap::Color,

    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get())]
    jobs: usize,

    remote_name: String,
    remote_uri: String,
}

fn try_main() -> anyhow::Result<ExitCode> {
    let args = Command::parse();

    // set color choice
    args.color.write_global();

    // create formatting subscriber that uses stderr
    let level = args.verbosity.log_level_filter();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level.as_trace())
        .with_writer(io::stderr)
        .with_ansi(colorize!(&io::stderr()));

    // initialize global subscriber
    subscriber.init();

    let mut stdout = anstream::stdout().lock();
    let stdin = io::stdin().lock();
    if stdin.is_terminal() {
        anyhow::bail!("requires running as a git pre-push hook");
    }

    // load repo from the current working directory
    let path = current_dir()?;
    let mut config = PkgcraftConfig::new("pkgcraft", "");
    let repo = config
        .add_format_repo_nested_path(&path, 0, RepoFormat::Ebuild)?
        .into_ebuild()
        .expect("failed loading repo");
    config
        .finalize()
        .map_err(|e| anyhow!("failed finalizing config: {e}"))?;

    // WARNING: This appears to invalidate the environment in some fashion so
    // std::env::var() calls don't work as expected after it even though
    // std::env::vars() will still show all the variables.
    let git_repo = git2::Repository::open(&path)
        .map_err(|e| anyhow!("failed opening git repo: {path}: {e}"))?;

    let mut failed = false;
    let mut reporter: Reporter = FancyReporter::default().into();
    let scanner = Scanner::new()
        .jobs(args.jobs)
        .exit([ReportLevel::Critical, ReportLevel::Error]);

    for line in stdin.lines() {
        let line = line?;
        // get hook input args
        let Some((_local_ref, local_obj, _remote_ref, remote_obj)) =
            line.split(' ').collect_tuple()
        else {
            anyhow::bail!("invalid pre-push hook arguments: {line}");
        };

        // determine diff
        let diff = git::diff(&git_repo, remote_obj, local_obj)?;

        // determine target Cpns from diff
        let mut cpns = IndexSet::new();
        let mut eclass = false;
        for delta in diff.deltas() {
            if let Some(path) = delta.new_file().path() {
                if let Ok(cpn) = repo.cpn_from_path(path) {
                    cpns.insert(cpn);
                } else if path.starts_with("eclass") {
                    eclass = true;
                }
            }
        }

        let mut reports = IndexSet::new();

        // scan individual packages that were changed
        for cpn in cpns {
            reports.extend(scanner.run(&repo, &cpn)?);
        }
        failed |= scanner.failed();

        // scan full tree for metadata errors on eclass changes
        if eclass {
            let scanner = scanner
                .clone()
                .reports([pkgcruft::check::CheckKind::Metadata]);
            // TODO: use eclass restriction instead of scanning entire repo
            reports.extend(scanner.run(&repo, Restrict::True)?);
            failed |= scanner.failed();
        }

        // output reports
        reports.sort();
        for report in reports {
            reporter.report(&report, &mut stdout)?;
        }
    }

    if failed {
        anyhow::bail!("scanning errors found")
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn main() -> anyhow::Result<ExitCode> {
    try_main().or_else(|e| {
        let cmd = env!("CARGO_BIN_NAME");
        eprintln!("{cmd}: error: {e}");
        Ok(ExitCode::from(1))
    })
}
