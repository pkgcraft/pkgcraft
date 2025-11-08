use std::process::ExitCode;

use clap::Args;
use clap::builder::ArgPredicate;
use pkgcraft::cli::{MaybeStdinVec, Targets};
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

    /// Output reports in sorted order
    #[arg(short, long)]
    sort: bool,

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

        // determine package restrictions
        let targets = Targets::new(&mut config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .pkg_targets(self.targets.iter().flatten())?
            .collapse();

        // create report scanner
        let scanner = Scanner::new()
            .jobs(self.jobs)
            .reports(self.reports.iter().copied())
            .filters(self.filters.iter().cloned())
            .force(self.force)
            .sort(self.sort)
            .exit(self.exit.iter().copied());

        // determine reporter
        let mut reporter = self.reporter.collapse(Some(&scanner));

        // run scanner for all targets
        let mut stdout = anstream::stdout().lock();
        for (repo, restrict) in targets.ebuild_repo_restricts() {
            // output reports
            for report in scanner.run(repo, restrict)? {
                reporter.report(&report, &mut stdout)?;
            }
        }

        reporter.finish(&mut stdout)?;
        Ok(ExitCode::from(scanner.failed() as u8))
    }
}
