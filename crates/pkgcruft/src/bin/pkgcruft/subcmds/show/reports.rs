use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use colored::Colorize;

use pkgcruft::report::REPORTS;

#[derive(Debug, Args)]
pub struct Subcommand {
    /// Color reports by level
    #[arg(short, long)]
    color: bool,
}

impl Subcommand {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let mut stdout = io::stdout().lock();
        for report in &*REPORTS {
            if self.color {
                writeln!(stdout, "{}", report.as_ref().color(report.level()))?;
            } else {
                writeln!(stdout, "{report}")?;
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
