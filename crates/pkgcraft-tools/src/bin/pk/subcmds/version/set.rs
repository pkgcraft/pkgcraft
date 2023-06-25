use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexSet;
use pkgcraft::dep::Version;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let versions: Result<IndexSet<_>, _> = self
            .vals
            .stdin_or_args()
            .split_whitespace()
            .map(|s| Version::new(&s))
            .collect();

        let mut handle = io::stdout().lock();
        for v in versions? {
            writeln!(handle, "{v}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
