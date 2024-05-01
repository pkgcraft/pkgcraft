use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::dep::Version;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub(crate) struct Command {
    values: Vec<String>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let versions: Vec<_> = self.values.stdin_or_args().split_whitespace().collect();
        let mut versions: Vec<_> = versions.iter().map(|s| Version::parse(s)).try_collect()?;

        versions.sort();

        let mut stdout = io::stdout().lock();
        for v in versions {
            writeln!(stdout, "{v}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
