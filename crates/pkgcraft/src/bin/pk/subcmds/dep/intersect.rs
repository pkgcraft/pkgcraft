use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::dep::PkgDep;

use crate::Run;

#[derive(Debug, Args)]
pub struct Intersect {
    dep1: String,
    dep2: String,
}

impl Run for Intersect {
    fn run(self) -> anyhow::Result<ExitCode> {
        let d1 = PkgDep::from_str(&self.dep1)?;
        let d2 = PkgDep::from_str(&self.dep2)?;
        Ok(ExitCode::from(!d1.intersects(&d2) as u8))
    }
}
