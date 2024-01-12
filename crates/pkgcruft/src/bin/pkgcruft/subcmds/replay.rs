use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::ExitCode;

use anyhow::anyhow;
use clap::Args;
use pkgcraft::restrict::{self, Restrict, Restriction};
use pkgcruft::report::{Report, ReportKind, REPORTS};
use pkgcruft::reporter::Reporter;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Replay options")]
pub struct Command {
    /// Reporter to use
    #[arg(short = 'R', long, default_value = "fancy")]
    reporter: Reporter,

    /// Limit to specific report variants
    #[arg(short, long, value_name = "REPORT")]
    reports: Vec<ReportKind>,

    /// Restriction to filter packages
    #[arg(short, long)]
    filter: Option<String>,

    // positionals
    /// Target file
    #[arg(default_value = "-", help_heading = "Arguments")]
    file: String,
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

        let mut reader: Box<dyn BufRead> = match self.file.as_str() {
            "-" => Box::new(io::stdin().lock()),
            path => {
                let file =
                    File::open(path).map_err(|e| anyhow!("failed loading file: {path}: {e}"))?;
                Box::new(BufReader::new(file))
            }
        };

        let mut line = String::new();
        while reader.read_line(&mut line)? != 0 {
            let report: Report =
                serde_json::from_str(&line).map_err(|e| anyhow!("invalid JSON report: {e}"))?;
            if reports.contains(report.kind()) && restrict.matches(&report) {
                self.reporter.report(&report)?;
            }
            line.clear();
        }

        Ok(ExitCode::SUCCESS)
    }
}
