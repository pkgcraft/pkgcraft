use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexSet;
use pkgcraft::dep::CpvOrDep;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub struct Command {
    values: Vec<String>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let values: Result<IndexSet<_>, _> = self
            .values
            .stdin_or_args()
            .split_whitespace()
            .map(|s| CpvOrDep::try_new(&s))
            .collect();

        let mut stdout = io::stdout().lock();
        for v in values? {
            writeln!(stdout, "{v}")?;
        }
        Ok(ExitCode::SUCCESS)
    }
}
