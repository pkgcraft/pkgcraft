use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Args, ValueHint};
use pkgcraft::restrict::{self, Restrict, Restriction};
use pkgcruft::report::{Report, ReportKind, REPORTS};
use pkgcruft::reporter::Reporter;
use strum::VariantNames;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Replay options")]
pub struct Command {
    /// Reporter to use
    #[arg(
        short = 'R',
        long,
        default_value = "fancy",
        hide_possible_values = true,
        value_parser = PossibleValuesParser::new(Reporter::VARIANTS)
            .map(|s| Reporter::from_str(&s).unwrap()),
    )]
    reporter: Reporter,

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
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let restrict = match self.filter.as_deref() {
            Some(s) => restrict::parse::dep(s)?,
            None => Restrict::True,
        };

        let reports: HashSet<_> = if self.reports.is_empty() {
            REPORTS.iter().collect()
        } else {
            self.reports.iter().collect()
        };

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
                self.reporter.report(&report)?;
            }
            line.clear();
        }

        Ok(ExitCode::SUCCESS)
    }
}
