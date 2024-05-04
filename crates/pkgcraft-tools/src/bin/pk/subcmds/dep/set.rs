use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::Dep;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub(crate) struct Command {
    values: Vec<String>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let values: IndexSet<_> = self
            .values
            .stdin_or_args()
            .split_whitespace()
            .map(Dep::try_new)
            .try_collect()?;

        let mut stdout = io::stdout().lock();
        for v in values {
            writeln!(stdout, "{v}")?;
        }
        Ok(ExitCode::SUCCESS)
    }
}
