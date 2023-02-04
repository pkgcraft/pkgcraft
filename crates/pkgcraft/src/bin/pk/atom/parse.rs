use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::atom::Atom;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Parse {
    atom: String,
}

impl Run for Parse {
    fn run(&self) -> anyhow::Result<ExitCode> {
        Atom::from_str(&self.atom)?;
        Ok(ExitCode::SUCCESS)
    }
}
