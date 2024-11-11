use std::collections::HashSet;
use std::io;
use std::process::ExitCode;

use clap::{Args, ValueHint};
use itertools::{Either, Itertools};
use pkgcraft::restrict::{self, Restrict};
use pkgcruft::report::{Iter, Report, ReportKind};

use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Replay options")]
pub(crate) struct Options {
    /// Package restrictions
    #[arg(short, long, value_name = "PKG[,...]", value_delimiter = ',')]
    pkgs: Vec<String>,

    /// Sort reports
    #[arg(long)]
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

    /// Target file paths
    #[arg(
        help_heading = "Arguments",
        value_hint = ValueHint::FilePath,
        required = true,
    )]
    files: Vec<String>,
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

    fn pkgs<I>(mut self, restricts: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let restricts: Vec<_> = restricts
            .into_iter()
            .map(|x| restrict::parse::dep(&x))
            .try_collect()?;

        self.pkgs = if restricts.is_empty() {
            None
        } else {
            Some(Restrict::or(restricts))
        };

        Ok(self)
    }

    fn run(
        &self,
        target: &str,
    ) -> anyhow::Result<impl Iterator<Item = pkgcruft::Result<Report>> + '_> {
        let reports = self.reports.as_ref();
        let pkgs = self.pkgs.as_ref();
        if target == "-" {
            let iter = Iter::from_reader(io::stdin().lock(), reports, pkgs);
            Ok(Either::Left(iter))
        } else {
            let iter = Iter::try_from_file(target, reports, pkgs)?;
            Ok(Either::Right(iter))
        }
    }
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        // determine enabled checks and reports
        let (_checks, reports) = self.checks.collapse(false)?;

        let replay = Replay::new().reports(reports).pkgs(self.options.pkgs)?;

        let mut reports = vec![];
        for file in &self.files {
            for report in replay.run(file)? {
                reports.push(report?);
            }
        }

        if self.options.sort {
            reports.sort();
        }

        let mut stdout = io::stdout().lock();
        let mut reporter = self.options.reporter.collapse();
        for report in reports {
            reporter.report(&report, &mut stdout)?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
