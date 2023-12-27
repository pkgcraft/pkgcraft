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
        let versions: Vec<_> = self.vals.stdin_or_args().split_whitespace().collect();
        let versions: Result<IndexSet<_>, _> = versions.iter().map(|s| Version::parse(s)).collect();

        let mut stdout = io::stdout().lock();
        for v in versions? {
            writeln!(stdout, "{v}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
