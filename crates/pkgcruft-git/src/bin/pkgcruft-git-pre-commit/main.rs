use std::process::ExitCode;
use std::{env, io};

use anyhow::anyhow;
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use indexmap::IndexSet;
use pkgcraft::config::Config as PkgcraftConfig;
use pkgcraft::repo::RepoFormat;
use pkgcraft::utils::current_dir;
use pkgcruft::report::ReportLevel;
use pkgcruft::reporter::{FancyReporter, Reporter};
use pkgcruft::scan::Scanner;
use tracing_log::AsTrace;

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    version,
    long_about = None,
    disable_help_subcommand = true,
)]
/// pkgcruft-git pre-commit hook
pub(crate) struct Command {
    #[command(flatten)]
    verbosity: Verbosity,

    #[command(flatten)]
    color: colorchoice_clap::Color,

    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get())]
    jobs: usize,
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Command::parse();

    // set color choice
    args.color.write_global();

    // create formatting subscriber that uses stderr
    let level = args.verbosity.log_level_filter();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level.as_trace())
        .with_writer(io::stderr);

    // initialize global subscriber
    subscriber.init();

    let mut stdout = anstream::stdout().lock();

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
    let git_repo = git2::Repository::open(repo.path())
        .map_err(|e| anyhow!("failed opening git repo: {path}: {e}"))?;

    let mut reporter: Reporter = FancyReporter::default().into();
    let scanner = Scanner::new()
        .jobs(args.jobs)
        .exit([ReportLevel::Critical, ReportLevel::Error]);

    // determine diff
    let tree = git_repo.head()?.peel_to_tree()?;
    let diff = git_repo.diff_tree_to_index(Some(&tree), None, None)?;

    // determine target Cpns from diff
    let mut cpns = IndexSet::new();
    for delta in diff.deltas() {
        if let Some(path) = delta.new_file().path()
            && let Ok(cpn) = repo.cpn_from_path(path)
        {
            cpns.insert(cpn);
        }
    }

    // scan individual packages that were changed
    for cpn in cpns {
        for report in scanner.run(&repo, &cpn)? {
            reporter.report(&report, &mut stdout)?;
        }
    }

    if scanner.failed() {
        anyhow::bail!("scanning errors found")
    } else {
        Ok(ExitCode::SUCCESS)
    }
}
