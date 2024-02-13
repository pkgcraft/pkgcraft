use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Cpv;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub(crate) struct Command {
    values: Vec<String>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let values: Result<Vec<_>, _> = self
            .values
            .stdin_or_args()
            .split_whitespace()
            .map(Cpv::try_new)
            .collect();

        let mut values = values?;
        values.sort();

        let mut stdout = io::stdout().lock();
        for v in values {
            writeln!(stdout, "{v}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
