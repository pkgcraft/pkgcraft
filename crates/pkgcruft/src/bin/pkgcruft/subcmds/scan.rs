use std::io;
use std::process::ExitCode;

use clap::builder::{ArgPredicate, PossibleValuesParser, TypedValueParser};
use clap::Args;
use pkgcraft::cli::{MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcruft::report::ReportKind;
use pkgcruft::scanner::Scanner;
use pkgcruft::source::PkgFilter;
use strum::VariantNames;

use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Scan options")]
pub(crate) struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repo
    #[arg(long)]
    repo: Option<String>,

    /// Exit status triggers
    #[arg(
        long,
        value_name = "REPORT[,...]",
        value_delimiter = ',',
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(ReportKind::VARIANTS)
            .map(|s| s.parse::<ReportKind>().unwrap()),
    )]
    exit: Vec<ReportKind>,

    /// Package filters
    #[arg(short, long, value_name = "FILTER[,...]")]
    filters: Vec<PkgFilter>,

    #[clap(flatten)]
    reporter: options::reporter::ReporterOptions,

    #[clap(flatten)]
    checks: options::checks::Checks,

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
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine enabled checks and reports
        let (checks, reports) = self.checks.collapse(true)?;

        // determine reporter
        let mut reporter = self.reporter.collapse();

        // determine target restrictions
        let targets = TargetRestrictions::new(config)
            .repo(self.repo)?
            .targets(self.targets.iter().flatten());

        // create report scanner
        let scanner = Scanner::new()
            .jobs(self.jobs.unwrap_or_default())
            .checks(checks)
            .reports(reports)
            .filters(self.filters)
            .exit(self.exit);

        // run scanner for all targets
        let mut stdout = io::stdout().lock();
        for target in targets {
            let (repo_set, restrict) = target?;
            for repo in repo_set {
                for report in scanner.run(&repo, &restrict)? {
                    reporter.report(&report, &mut stdout)?;
                }
            }
        }

        let failed: u8 = scanner.failed().into();
        Ok(failed.into())
    }
}
