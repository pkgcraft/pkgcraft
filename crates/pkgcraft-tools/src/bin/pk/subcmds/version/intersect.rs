use std::process::ExitCode;

use pkgcraft::dep::Version;
use pkgcraft::traits::Intersects;

#[derive(Debug, clap::Args)]
pub struct Command {
    value1: String,
    value2: String,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let v1 = Version::parse(&self.value1)?;
        let v2 = Version::parse(&self.value2)?;
        Ok(ExitCode::from(!v1.intersects(&v2) as u8))
    }
}
