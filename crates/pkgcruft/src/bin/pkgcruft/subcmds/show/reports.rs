use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use colored::Colorize;
use pkgcruft::report::ReportKind;
use strum::IntoEnumIterator;

use crate::options;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Report options")]
pub(super) struct Subcommand {
    #[clap(flatten)]
    reports: options::reports::Reports,
}

impl Subcommand {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let reports = if self.reports.is_empty() {
            ReportKind::iter().collect()
        } else {
            self.reports.replay().unwrap_or_default()
        };

        let mut stdout = io::stdout().lock();
        for report in reports {
            writeln!(stdout, "{}", report.as_ref().color(report.level()))?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
