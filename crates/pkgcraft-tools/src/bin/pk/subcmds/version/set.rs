use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexSet;
use pkgcraft::dep::Version;

use crate::args::stdin_or_args;

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let versions: Result<IndexSet<_>, _> =
            stdin_or_args(self.vals).map(|s| Version::new(&s)).collect();

        let mut handle = io::stdout().lock();
        for v in versions? {
            writeln!(handle, "{v}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
