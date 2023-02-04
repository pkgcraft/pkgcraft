use std::process::ExitCode;
use std::str::FromStr;

use pkgcraft::atom::Atom;

use crate::Run;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    atom: String,
}

impl Run for Args {
    fn run(&self) -> anyhow::Result<ExitCode> {
        Atom::from_str(&self.atom)?;
        Ok(ExitCode::SUCCESS)
    }
}
