use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::MaybeStdinVec;
use pkgcraft::dep::Version;

#[derive(Args)]
pub(crate) struct Command {
    values: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let mut values: Vec<_> = self
            .values
            .iter()
            .flatten()
            .flat_map(|s| s.split_whitespace())
            .map(Version::try_new)
            .try_collect()?;

        values.sort();

        let mut stdout = io::stdout().lock();
        for v in values {
            writeln!(stdout, "{v}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
