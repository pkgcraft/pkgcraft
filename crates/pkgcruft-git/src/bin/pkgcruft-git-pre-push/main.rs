use std::env;
use std::io::{self, BufRead, IsTerminal};
use std::process::ExitCode;

use anyhow::anyhow;
use camino::Utf8Path;
use clap::Parser;
use clap_verbosity_flag::{Verbosity, log::LevelFilter};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcraft::restrict::Restrict;
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

    /// enable/disable color support
    #[arg(long, value_name = "BOOL", hide_possible_values = true)]
    color: Option<bool>,

    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get())]
    jobs: usize,

    remote_name: String,
    remote_uri: String,
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Command::parse();

    // custom log event formatter that disables target prefixes by default
    let level = args.verbosity.log_level_filter();
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(level > LevelFilter::Info)
        .without_time()
        .compact();

    // create formatting subscriber that uses stderr
    let mut subscriber = tracing_subscriber::fmt()
        .event_format(format)
        .with_max_level(level.as_trace())
        .with_writer(io::stderr);

    // forcibly enable or disable subscriber output color
    if let Some(value) = args.color {
        subscriber = subscriber.with_ansi(value);
    }

    // initialize global subscriber
    subscriber.init();

    let mut stdout = io::stdout().lock();
    let stdin = io::stdin().lock();
    if stdin.is_terminal() {
        anyhow::bail!("requires running as a git pre-push hook");
    }

    // git appears to mangle $PWD to the repo's root path instead of exporting $GIT_DIR or
    // $GIT_WORK_TREE like other hooks use.
    let path = env::var("PWD")?;
    let mut config = PkgcraftConfig::new("pkgcraft", "");
    let repo = config
        .add_repo_path("repo", &path, 0)
        .map_err(|e| anyhow!("invalid repo: {e}"))?;
    let repo = repo
        .into_ebuild()
        .map_err(|e| anyhow!("invalid ebuild repo: {path}: {e}"))?;
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
        .exit([ReportLevel::Critical, ReportLevel::Warning]);

    for line in stdin.lines() {
        let line = line?;
        // get hook arguments
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
            if let Some(path) = delta.new_file().path().and_then(Utf8Path::from_path) {
                if let Ok(cpn) = repo.cpn_from_path(path) {
                    cpns.insert(cpn);
                } else if path.as_str().starts_with("eclass/") {
                    eclass = true;
                }
            }
        }

        // scan individual packages that were changed
        for cpn in cpns {
            for report in scanner.run(&repo, &cpn)? {
                reporter.report(&report, &mut stdout)?;
            }
        }
        failed |= scanner.failed();

        // scan full tree for metadata errors on eclass changes
        if eclass {
            let scanner = scanner.clone().reports([pkgcruft::check::Check::Metadata]);
            for report in scanner.run(&repo, Restrict::True)? {
                reporter.report(&report, &mut stdout)?;
            }
            failed |= scanner.failed();
        }
    }

    if failed {
        anyhow::bail!("scanning errors found")
    } else {
        Ok(ExitCode::SUCCESS)
    }
}
