use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::{CpvOrDep, Intersects};

#[derive(Debug, Args)]
pub struct Command {
    dep1: String,
    dep2: String,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let obj1: CpvOrDep = self.dep1.parse()?;
        let obj2: CpvOrDep = self.dep2.parse()?;
        Ok(ExitCode::from(!obj1.intersects(&obj2) as u8))
    }
}
