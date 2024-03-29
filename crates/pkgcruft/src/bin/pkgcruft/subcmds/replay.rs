use std::collections::HashSet;
use std::io;
use std::process::ExitCode;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, ValueHint};
use itertools::Either;
use pkgcraft::restrict::{self, Restrict};
use pkgcruft::report::{Iter, Report, ReportKind, REPORTS};

use crate::options::reporter::ReporterOptions;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Replay options")]
pub(crate) struct Command {
    /// Limit to specific report variants
    #[arg(
        short,
        long,
        value_name = "REPORT",
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(REPORTS.iter().map(|r| r.as_ref()))
            .map(|s| s.parse::<ReportKind>().unwrap()),
    )]
    reports: Vec<ReportKind>,

    /// Restriction to filter packages
    #[arg(short, long)]
    filter: Option<String>,

    /// Sort reports
    #[arg(short, long)]
    sort: bool,

    #[clap(flatten)]
    reporter: ReporterOptions,

    // positionals
    /// Target file path
    #[arg(
        help_heading = "Arguments",
        value_name = "FILE",
        value_hint = ValueHint::FilePath,
        default_value = "-",
    )]
    file: String,
}

#[derive(Debug)]
struct Replay {
    reports: HashSet<ReportKind>,
    filter: Restrict,
}

impl Replay {
    fn new() -> Self {
        Self {
            reports: REPORTS.iter().copied().collect(),
            filter: Restrict::True,
        }
    }

    fn reports(mut self, reports: Vec<ReportKind>) -> Self {
        if !reports.is_empty() {
            self.reports = reports.into_iter().collect();
        }
        self
    }

    fn filter(mut self, restrict: Option<String>) -> anyhow::Result<Self> {
        if let Some(s) = restrict.as_deref() {
            self.filter = restrict::parse::dep(s)?;
        };
        Ok(self)
    }

    fn run(
        &self,
        target: String,
    ) -> anyhow::Result<impl Iterator<Item = pkgcruft::Result<Report>> + '_> {
        let filters = (&self.reports, &self.filter);
        if target == "-" {
            let iter = Iter::from_reader(io::stdin().lock(), Some(filters));
            Ok(Either::Left(iter))
        } else {
            let iter = Iter::try_from_file(&target, Some(filters))?;
            Ok(Either::Right(iter))
        }
    }
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let replay = Replay::new().reports(self.reports).filter(self.filter)?;

        let mut stdout = io::stdout().lock();
        let mut reporter = self.reporter.collapse()?;

        if self.sort {
            let reports: Result<Vec<_>, _> = replay.run(self.file)?.collect();
            let mut reports = reports?;
            reports.sort();
            for report in reports {
                reporter.report(&report, &mut stdout)?;
            }
        } else {
            for report in replay.run(self.file)? {
                reporter.report(&(report?), &mut stdout)?;
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
