use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;

use pkgcruft::check::Check;

#[derive(Debug, Args)]
pub(super) struct Subcommand;

impl Subcommand {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let mut stdout = io::stdout().lock();
        for check in Check::iter() {
            writeln!(stdout, "{check}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
