use std::process::ExitCode;
use std::str::FromStr;

use pkgcraft::atom::Version;

use crate::Run;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    version: String,
}

impl Run for Args {
    fn run(&self) -> anyhow::Result<ExitCode> {
        Version::from_str(&self.version)?;
        Ok(ExitCode::SUCCESS)
    }
}
