use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::dep::Version;

use crate::Run;

#[derive(Debug, Args)]
pub struct Intersect {
    version1: String,
    version2: String,
}

impl Run for Intersect {
    fn run(self) -> anyhow::Result<ExitCode> {
        let v1 = Version::from_str(&self.version1)?;
        let v2 = Version::from_str(&self.version2)?;
        Ok(ExitCode::from(!v1.intersects(&v2) as u8))
    }
}
