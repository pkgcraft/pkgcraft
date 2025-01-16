use std::io;
use std::process::ExitCode;

use clap::builder::ArgPredicate;
use clap::Args;
use pkgcraft::cli::{MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;
use pkgcruft::report::ReportSet;
use pkgcruft::scan::Scanner;
use pkgcruft::source::PkgFilter;

use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Scan options")]
pub(crate) struct Command {
    /// Enable package filtering
    #[arg(short, long, value_name = "FILTER[,...]")]
    filters: Vec<PkgFilter>,

    /// Disregard ignore settings
    #[arg(short = 'F', long)]
    force: bool,

    /// Parallel jobs to run
    #[arg(short, long, default_value_t = num_cpus::get())]
    jobs: usize,

    /// Exit status triggers
    #[arg(
        long,
        value_name = "SET[,...]",
        value_delimiter = ',',
        num_args = 0..=1,
        default_missing_value = "@critical,@error",
    )]
    exit: Vec<ReportSet>,

    /// Target repo
    #[arg(long)]
    repo: Option<String>,

    #[clap(flatten)]
    reporter: options::reporter::ReporterOptions,

    #[clap(flatten)]
    reports: options::reports::Reports,

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
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let mut config = Config::new("pkgcraft", "");

        // determine reporter
        let mut reporter = self.reporter.collapse();

        // determine target restrictions
        let targets = TargetRestrictions::new(&mut config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .finalize_targets(self.targets.iter().flatten())?;

        // run scanner for all targets
        let mut failed = false;
        let mut stdout = io::stdout().lock();
        for (repo, restrict) in targets.ebuild_repo_restricts() {
            // create report scanner
            let scanner = Scanner::new(repo)
                .jobs(self.jobs)
                .reports(self.reports.iter().copied())
                .filters(self.filters.iter().cloned())
                .force(self.force)
                .exit(self.exit.iter().copied());

            // output reports
            for report in scanner.run(restrict)? {
                reporter.report(&report, &mut stdout)?;
            }

            // track failure status
            failed |= scanner.failed();
        }

        reporter.finish(&mut stdout)?;
        Ok(ExitCode::from(failed as u8))
    }
}
