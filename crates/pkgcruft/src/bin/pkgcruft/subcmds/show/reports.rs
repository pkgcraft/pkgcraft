use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;

use pkgcruft::report::REPORTS;

#[derive(Debug, Args)]
pub struct Subcommand {}

impl Subcommand {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let mut stdout = io::stdout().lock();
        for report in &*REPORTS {
            writeln!(stdout, "{report}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
