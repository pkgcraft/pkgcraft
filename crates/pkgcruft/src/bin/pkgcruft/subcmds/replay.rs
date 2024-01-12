use std::collections::HashSet;
use std::io::{self, BufRead};
use std::process::ExitCode;

use clap::Args;
use pkgcruft::report::{Report, ReportKind, REPORTS};
use pkgcruft::reporter::Reporter;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Replay options")]
pub struct Command {
    /// Reporter to use
    #[arg(short, long, default_value = "fancy")]
    reporter: Reporter,

    /// Limit to specific report variants
    #[arg(short, long, value_name = "REPORT")]
    filter: Vec<ReportKind>,
}

impl Command {
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let filter: HashSet<_> = if self.filter.is_empty() {
            REPORTS.iter().collect()
        } else {
            self.filter.iter().collect()
        };

        let mut lock = io::stdin().lock();
        let mut line = String::new();
        while lock.read_line(&mut line)? != 0 {
            let report: Report = serde_json::from_str(&line).unwrap();
            if filter.contains(report.kind()) {
                self.reporter.report(&report)?;
            }
            line.clear();
        }

        Ok(ExitCode::SUCCESS)
    }
}
