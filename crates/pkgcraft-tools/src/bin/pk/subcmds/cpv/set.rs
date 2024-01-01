use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexSet;
use pkgcraft::dep::Cpv;

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
            .map(Cpv::try_new)
            .collect();

        let mut stdout = io::stdout().lock();
        for v in values? {
            writeln!(stdout, "{v}")?;
        }
        Ok(ExitCode::SUCCESS)
    }
}
