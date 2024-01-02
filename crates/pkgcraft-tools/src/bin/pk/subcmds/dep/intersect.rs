use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::CpvOrDep;
use pkgcraft::traits::Intersects;

#[derive(Debug, Args)]
pub struct Command {
    value1: String,
    value2: String,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let obj1 = CpvOrDep::parse(&self.value1)?;
        let obj2 = CpvOrDep::parse(&self.value2)?;
        Ok(ExitCode::from(!obj1.intersects(&obj2) as u8))
    }
}
