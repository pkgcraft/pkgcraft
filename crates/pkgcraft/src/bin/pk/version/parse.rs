use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::atom::Version;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Parse {
    version: String,
}

impl Run for Parse {
    fn run(self) -> anyhow::Result<ExitCode> {
        Version::from_str(&self.version)?;
        Ok(ExitCode::SUCCESS)
    }
}
