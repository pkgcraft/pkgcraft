use std::cmp::Ordering;
use std::fmt;
use std::io::{self, Write};
use std::process::ExitCode;

use camino::{Utf8Path, Utf8PathBuf};
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
    #[arg(short, long)]
    sort: bool,
}

#[derive(Debug, Args)]
pub(crate) struct Command {
    #[clap(flatten)]
    reports: options::reports::Reports,

    #[clap(flatten)]
    options: Options,

    /// Old file path
    #[arg(help_heading = "Arguments", display_order = 0, value_hint = ValueHint::FilePath)]
    old: Utf8PathBuf,

    /// New file path
    #[arg(help_heading = "Arguments", display_order = 1, value_hint = ValueHint::FilePath)]
    new: Utf8PathBuf,
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
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let restricts: Vec<_> = restricts
            .into_iter()
            .map(restrict::parse::dep)
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
        target: &Utf8Path,
    ) -> anyhow::Result<impl Iterator<Item = pkgcruft::Result<Report>> + '_> {
        let iter =
            Iter::try_from_file(target, self.reports.as_ref(), self.pkgs.as_ref(), None)?;
        Ok(iter)
    }
}

/// Report difference variants.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
enum ChangeKind {
    Removed,
    Added,
}

impl fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Removed => write!(f, "-"),
            Self::Added => write!(f, "+"),
        }
    }
}

/// Wrapper for report differences.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
struct Change<'a> {
    kind: ChangeKind,
    report: &'a Report,
}

impl<'a> Change<'a> {
    fn removed(report: &'a Report) -> Self {
        Self {
            kind: ChangeKind::Removed,
            report,
        }
    }

    fn added(report: &'a Report) -> Self {
        Self {
            kind: ChangeKind::Added,
            report,
        }
    }
}

impl Ord for Change<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.report
            .cmp(other.report)
            .then_with(|| self.kind.cmp(&other.kind))
    }
}

impl PartialOrd for Change<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Change<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let color = match self.kind {
            ChangeKind::Removed => Color::Red,
            ChangeKind::Added => Color::Green,
        };

        let kind = &self.kind;
        let report = &self.report;
        write!(f, "{}", format!("{kind}{report}").color(color))
    }
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        // determine enabled reports
        let enabled = self.reports.replay()?;

        let replay = Replay::new().reports(enabled).pkgs(&self.options.pkgs)?;

        let old: IndexSet<_> = replay.run(&self.old)?.try_collect()?;
        let new: IndexSet<_> = replay.run(&self.new)?.try_collect()?;
        let removed = old.difference(&new).map(Change::removed);
        let added = new.difference(&old).map(Change::added);
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
