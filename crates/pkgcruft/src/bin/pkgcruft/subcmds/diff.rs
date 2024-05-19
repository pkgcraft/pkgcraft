use std::collections::HashSet;
use std::io;
use std::process::ExitCode;

use clap::{Args, ValueHint};
use itertools::Itertools;
use pkgcraft::restrict::{self, Restrict};
use pkgcruft::report::{Iter, Report, ReportKind};

use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Diff options")]
pub(crate) struct Options {
    /// Package restriction
    #[arg(short, long)]
    pkgs: Option<String>,

    /// Sort reports
    #[arg(short, long)]
    sort: bool,

    #[clap(flatten)]
    reporter: options::reporter::ReporterOptions,
}

#[derive(Debug, Args)]
pub(crate) struct Command {
    #[clap(flatten)]
    checks: options::checks::Checks,

    #[clap(flatten)]
    options: Options,

    /// Target file path
    #[arg(help_heading = "Arguments", value_hint = ValueHint::FilePath)]
    file1: String,

    /// Target file path
    #[arg(help_heading = "Arguments", value_hint = ValueHint::FilePath)]
    file2: String,
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

    fn reports<I>(mut self, reports: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.reports = Some(reports.into_iter().collect());
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
        let iter = Iter::try_from_file(&target, self.reports.as_ref(), self.pkgs.as_ref())?;
        Ok(iter)
    }
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        // determine enabled checks and reports
        let (_checks, reports) = self.checks.collapse();

        let replay = Replay::new().reports(reports).pkgs(self.options.pkgs)?;

        let reports1: HashSet<_> = replay.run(self.file1)?.try_collect()?;
        let reports2: HashSet<_> = replay.run(self.file2)?.try_collect()?;
        let mut reports: Vec<_> = reports1.symmetric_difference(&reports2).collect();

        if self.options.sort {
            reports.sort();
        }

        // TODO: output context for reports from the first file vs the second file
        let mut stdout = io::stdout().lock();
        let mut reporter = self.options.reporter.collapse();
        for report in reports {
            reporter.report(report, &mut stdout)?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
