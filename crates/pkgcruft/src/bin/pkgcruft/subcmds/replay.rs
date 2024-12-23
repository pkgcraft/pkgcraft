use std::io;
use std::process::ExitCode;

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, ValueHint};
use indexmap::IndexSet;
use itertools::{Either, Itertools};
use pkgcraft::restrict::{self, Restrict};
use pkgcruft::report::{Iter, Report, ReportKind};
use pkgcruft::scope::Scope;
use strum::{IntoEnumIterator, VariantNames};

use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Replay options")]
pub(crate) struct Options {
    /// Package restrictions
    #[arg(short, long, value_name = "PKG[,...]", value_delimiter = ',')]
    pkgs: Vec<String>,

    /// Restrict by scope
    #[arg(
        short,
        long,
        value_name = "SCOPE[,...]",
        value_delimiter = ',',
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(Scope::VARIANTS)
            .map(|s| s.parse::<Scope>().unwrap()),
    )]
    scopes: Vec<Scope>,

    /// Sort reports
    #[arg(long)]
    sort: bool,

    #[clap(flatten)]
    reporter: options::reporter::ReporterOptions,
}

#[derive(Debug, Args)]
pub(crate) struct Command {
    #[clap(flatten)]
    reports: options::reports::Reports,

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
    reports: Option<IndexSet<ReportKind>>,
    scopes: Option<IndexSet<Scope>>,
    pkgs: Option<Restrict>,
}

impl Replay {
    fn new() -> Self {
        Self::default()
    }

    fn reports<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.reports = Some(values.into_iter().collect());
        self
    }

    fn pkgs<I>(mut self, values: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let restricts: Vec<_> = values
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

    fn scopes<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = Scope>,
    {
        self.scopes = Some(values.into_iter().collect());
        self
    }

    fn run(
        &self,
        target: &str,
    ) -> anyhow::Result<impl Iterator<Item = pkgcruft::Result<Report>> + '_> {
        let reports = self.reports.as_ref();
        let pkgs = self.pkgs.as_ref();
        let scopes = self.scopes.as_ref();
        if target == "-" {
            let iter = Iter::from_reader(io::stdin().lock(), reports, pkgs, scopes);
            Ok(Either::Left(iter))
        } else {
            let iter = Iter::try_from_file(target, reports, pkgs, scopes)?;
            Ok(Either::Right(iter))
        }
    }
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        // determine enabled reports
        let defaults = ReportKind::iter().collect();
        let (enabled, _) = self.reports.collapse(defaults)?;

        let mut replay = Replay::new().reports(enabled).pkgs(self.options.pkgs)?;

        if !self.options.scopes.is_empty() {
            replay = replay.scopes(self.options.scopes);
        }

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

        reporter.finish(&mut stdout)?;
        Ok(ExitCode::SUCCESS)
    }
}
