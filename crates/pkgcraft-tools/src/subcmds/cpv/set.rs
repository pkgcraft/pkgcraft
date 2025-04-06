use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::cli::MaybeStdinVec;
use pkgcraft::dep::Cpv;

#[derive(Args)]
pub(crate) struct Command {
    values: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let values: IndexSet<_> = self
            .values
            .iter()
            .flatten()
            .flat_map(|s| s.split_whitespace())
            .map(Cpv::try_new)
            .try_collect()?;

        let mut stdout = io::stdout().lock();
        for v in values {
            writeln!(stdout, "{v}")?;
        }
        Ok(ExitCode::SUCCESS)
    }
}
