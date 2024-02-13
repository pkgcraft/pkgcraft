use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Dep;

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
            .map(|s| Dep::try_new(&s))
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
