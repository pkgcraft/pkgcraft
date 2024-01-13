use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::ExitCode;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::builder::PossibleValuesParser;
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
        value_parser = PossibleValuesParser::new(REPORTS.iter().map(|r| r.as_ref())),
    )]
    reports: Vec<ReportKind>,

    /// Restriction to filter packages
    #[arg(short, long)]
    filter: Option<String>,

    #[clap(flatten)]
    reporter: ReporterOptions,

    // positionals
    /// Target file path (uses stdin by default)
    #[arg(
        help_heading = "Arguments",
        value_name = "FILE",
        value_hint = ValueHint::FilePath,
    )]
    file: Option<Utf8PathBuf>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        // determine package restriction
        let restrict = match self.filter.as_deref() {
            Some(s) => restrict::parse::dep(s)?,
            None => Restrict::True,
        };

        // determine reports filter
        let reports: HashSet<_> = if self.reports.is_empty() {
            REPORTS.iter().collect()
        } else {
            self.reports.iter().collect()
        };

        // determine reporter
        let mut reporter = self.reporter.collapse()?;

        // open target file for reading
        let mut reader: Box<dyn BufRead> = match self.file.as_ref() {
            None => Box::new(io::stdin().lock()),
            Some(path) => {
                let file =
                    File::open(path).map_err(|e| anyhow!("failed loading file: {path}: {e}"))?;
                Box::new(BufReader::new(file))
            }
        };

        let mut line = String::new();
        while reader.read_line(&mut line)? != 0 {
            let report = Report::from_json(&line)?;
            if reports.contains(report.kind()) && restrict.matches(&report) {
                reporter.report(&report)?;
            }
            line.clear();
        }

        Ok(ExitCode::SUCCESS)
    }
}
