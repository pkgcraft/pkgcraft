use std::collections::HashSet;
use std::io;
use std::process::ExitCode;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, ValueHint};
use itertools::{Either, Itertools};
use pkgcraft::restrict::{self, Restrict};
use pkgcruft::report::{Iter, Report, ReportKind, REPORTS};

use crate::options::reporter::ReporterOptions;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Replay options")]
pub(crate) struct Options {
    /// Limit to specific report variants
    #[arg(
        short,
        long,
        value_name = "REPORT",
        value_delimiter = ',',
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(REPORTS.iter().map(|r| r.as_ref()))
            .map(|s| s.parse::<ReportKind>().unwrap()),
    )]
    reports: Vec<ReportKind>,

    /// Package restriction
    #[arg(short, long)]
    pkgs: Option<String>,

    /// Sort reports
    #[arg(short, long)]
    sort: bool,

    #[clap(flatten)]
    reporter: ReporterOptions,
}

#[derive(Debug, Args)]
pub(crate) struct Command {
    #[clap(flatten)]
    options: Options,

    /// Target file path
    #[arg(
        help_heading = "Arguments",
        value_hint = ValueHint::FilePath,
        default_value = "-",
    )]
    file: String,
}

#[derive(Debug, Default)]
struct Replay {
    reports: Option<HashSet<ReportKind>>,
    pkgs: Option<Restrict>,
}

impl Replay {
    fn new() -> Self {
        Self::default()
    }

    fn reports(mut self, reports: Vec<ReportKind>) -> Self {
        if !reports.is_empty() {
            self.reports = Some(reports.into_iter().collect());
        }
        self
    }

    fn pkgs(mut self, restrict: Option<String>) -> anyhow::Result<Self> {
        if let Some(s) = restrict.as_deref() {
            self.pkgs = Some(restrict::parse::dep(s)?);
        };
        Ok(self)
    }

    fn run(
        &self,
        target: String,
    ) -> anyhow::Result<impl Iterator<Item = pkgcruft::Result<Report>> + '_> {
        let reports = self.reports.as_ref();
        let pkgs = self.pkgs.as_ref();
        if target == "-" {
            let iter = Iter::from_reader(io::stdin().lock(), reports, pkgs);
            Ok(Either::Left(iter))
        } else {
            let iter = Iter::try_from_file(&target, reports, pkgs)?;
            Ok(Either::Right(iter))
        }
    }
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let replay = Replay::new()
            .reports(self.options.reports)
            .pkgs(self.options.pkgs)?;

        let reports = if self.options.sort {
            let mut reports: Vec<_> = replay.run(self.file)?.try_collect()?;
            reports.sort();
            Either::Left(reports.into_iter().map(Ok))
        } else {
            Either::Right(replay.run(self.file)?)
        };

        let mut stdout = io::stdout().lock();
        let mut reporter = self.options.reporter.collapse()?;
        for report in reports {
            reporter.report(&(report?), &mut stdout)?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
