use std::io;
use std::process::ExitCode;

use clap::builder::{ArgPredicate, PossibleValuesParser, TypedValueParser};
use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::{MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcruft::report::{ReportKind, ReportLevel};
use pkgcruft::scanner::Scanner;
use pkgcruft::source::PkgFilter;
use strum::{IntoEnumIterator, VariantNames};

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
        num_args = 0..=1,
        default_missing_value = ReportKind::iter()
            .filter(|x| x.level() <= ReportLevel::Error).join(","),
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
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // load system-defined repos
        config.load()?;

        // determine reporter
        let mut reporter = self.reporter.collapse();

        // determine target restrictions
        let targets: Vec<_> = TargetRestrictions::new(config)
            .repo(self.repo.as_deref())?
            .targets(self.targets.iter().flatten())
            .try_collect()?;
        config.finalize()?;

        // run scanner for all targets
        let mut failed = false;
        let mut stdout = io::stdout().lock();
        for (repo_set, restrict) in targets {
            for repo in repo_set {
                let repo = repo
                    .as_ebuild()
                    .ok_or_else(|| anyhow::anyhow!("non-ebuild repo: {repo}"))?;

                // determine enabled checks and reports
                let (checks, reports) = self.checks.collapse(Some(repo))?;

                // create report scanner
                let scanner = Scanner::new(repo)
                    .jobs(self.jobs.unwrap_or_default())
                    .checks(checks)
                    .reports(reports)
                    .filters(self.filters.iter().cloned())
                    .exit(self.exit.iter().copied());

                // output reports
                for report in scanner.run(&restrict)? {
                    reporter.report(&report, &mut stdout)?;
                }

                // track failure status
                failed |= scanner.failed();
            }
        }

        reporter.finish(&mut stdout)?;
        Ok(ExitCode::from(failed as u8))
    }
}
