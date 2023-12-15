use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::CpvOrDep;
use pkgcraft::traits::Intersects;

#[derive(Debug, Args)]
pub struct Command {
    dep1: String,
    dep2: String,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let obj1 = CpvOrDep::parse(&self.dep1)?;
        let obj2 = CpvOrDep::parse(&self.dep2)?;
        Ok(ExitCode::from(!obj1.intersects(&obj2) as u8))
    }
}
