use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Version;

use crate::Run;

#[derive(Debug, Args)]
pub struct Intersect {
    ver1: String,
    ver2: String,
}

impl Run for Intersect {
    fn run(self) -> anyhow::Result<ExitCode> {
        let v1 = Version::new(&self.ver1).or_else(|_| Version::new_with_op(&self.ver1))?;
        let v2 = Version::new(&self.ver2).or_else(|_| Version::new_with_op(&self.ver2))?;
        Ok(ExitCode::from(!v1.intersects(&v2) as u8))
    }
}
