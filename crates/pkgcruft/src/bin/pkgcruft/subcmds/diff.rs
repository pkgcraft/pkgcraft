use std::cmp::Ordering;
use std::fmt;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Args, ValueHint};
use colored::{Color, Colorize};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::restrict::{self, Restrict};
use pkgcruft::report::{Iter, Report, ReportKind};

use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Diff options")]
pub(crate) struct Options {
    /// Package restrictions
    #[arg(short, long, value_name = "PKG[,...]", value_delimiter = ',')]
    pkgs: Vec<String>,

    /// Sort reports
    #[arg(long)]
    sort: bool,
}

#[derive(Debug, Args)]
pub(crate) struct Command {
    #[clap(flatten)]
    checks: options::checks::Checks,

    #[clap(flatten)]
    options: Options,

    /// Old file path
    #[arg(help_heading = "Arguments", display_order = 0, value_hint = ValueHint::FilePath)]
    old: String,

    /// New file path
    #[arg(help_heading = "Arguments", display_order = 1, value_hint = ValueHint::FilePath)]
    new: String,
}

#[derive(Debug, Default)]
struct Replay {
    reports: Option<IndexSet<ReportKind>>,
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
        target: String,
    ) -> anyhow::Result<impl Iterator<Item = pkgcruft::Result<Report>> + '_> {
        let iter = Iter::try_from_file(&target, self.reports.as_ref(), self.pkgs.as_ref())?;
        Ok(iter)
    }
}

/// Wrapper for report differences.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
enum Change<'a> {
    Removed(&'a Report),
    Added(&'a Report),
}

impl Change<'_> {
    fn report(&self) -> &Report {
        match self {
            Self::Removed(report) => report,
            Self::Added(report) => report,
        }
    }
}

impl Ord for Change<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.report()
            .cmp(other.report())
            .then_with(|| match (self, other) {
                (Self::Removed(_), Self::Added(_)) => Ordering::Less,
                (Self::Added(_), Self::Removed(_)) => Ordering::Greater,
                _ => Ordering::Equal,
            })
    }
}

impl PartialOrd for Change<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Change<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Removed(report) => write!(f, "{}", format!("-{report}").color(Color::Red)),
            Self::Added(report) => write!(f, "{}", format!("+{report}").color(Color::Green)),
        }
    }
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        // determine enabled checks and reports
        let (_checks, reports) = self.checks.collapse(None)?;

        let replay = Replay::new().reports(reports).pkgs(self.options.pkgs)?;

        let old: IndexSet<_> = replay.run(self.old)?.try_collect()?;
        let new: IndexSet<_> = replay.run(self.new)?.try_collect()?;
        let removed = old.difference(&new).map(Change::Removed);
        let added = new.difference(&old).map(Change::Added);
        let mut changes: Vec<_> = removed.chain(added).collect();

        if self.options.sort {
            changes.sort();
        }

        let mut stdout = io::stdout().lock();
        for change in changes {
            writeln!(stdout, "{change}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
