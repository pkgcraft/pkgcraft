use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;

use pkgcruft::check::CHECKS;

#[derive(Debug, Args)]
pub struct Subcommand {}

impl Subcommand {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let mut stdout = io::stdout().lock();
        for check in &*CHECKS {
            writeln!(stdout, "{check}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
