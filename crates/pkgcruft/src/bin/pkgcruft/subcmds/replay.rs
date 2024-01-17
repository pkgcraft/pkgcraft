use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::ExitCode;

use anyhow::anyhow;
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, ValueHint};
use pkgcraft::restrict::{self, Restrict, Restriction};
use pkgcruft::report::{Report, ReportKind, REPORTS};

use crate::options::reporter::ReporterOptions;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Replay options")]
pub struct Command {
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
    ) -> anyhow::Result<Box<dyn Iterator<Item = anyhow::Result<Report>> + '_>> {
        if target == "-" {
            Ok(Box::new(Iter {
                line: String::new(),
                reader: io::stdin().lock(),
                reports: &self.reports,
                filter: &self.filter,
            }))
        } else {
            let file =
                File::open(&target).map_err(|e| anyhow!("failed loading file: {target}: {e}"))?;
            Ok(Box::new(Iter {
                line: String::new(),
                reader: BufReader::new(file),
                reports: &self.reports,
                filter: &self.filter,
            }))
        }
    }
}

struct Iter<'a, R: BufRead> {
    line: String,
    reader: R,
    reports: &'a HashSet<ReportKind>,
    filter: &'a Restrict,
}

impl<R: BufRead> Iterator for Iter<'_, R> {
    type Item = anyhow::Result<Report>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.line.clear();
            match self.reader.read_line(&mut self.line) {
                Ok(0) => return None,
                Ok(_) => match Report::from_json(&self.line) {
                    Ok(report) => {
                        if self.reports.contains(report.kind()) && self.filter.matches(&report) {
                            return Some(Ok(report));
                        }
                    }
                    Err(e) => return Some(Err(anyhow!("{e}"))),
                },
                Err(e) => return Some(Err(anyhow!("failed reading line: {e}"))),
            }
        }
    }
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let replay = Replay::new().reports(self.reports).filter(self.filter)?;

        let mut stdout = io::stdout().lock();
        let mut reporter = self.reporter.collapse()?;
        for report in replay.run(self.file)? {
            reporter.report(&(report?), &mut stdout)?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
