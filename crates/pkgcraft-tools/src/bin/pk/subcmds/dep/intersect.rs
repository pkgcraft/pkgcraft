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
        let obj1: CpvOrDep<_> = self.dep1.parse()?;
        let obj2: CpvOrDep<_> = self.dep2.parse()?;
        Ok(ExitCode::from(!obj1.intersects(&obj2) as u8))
    }
}
