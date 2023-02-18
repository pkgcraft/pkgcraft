use std::process::ExitCode;

use clap::Args;

use crate::Run;

#[derive(Debug, Args)]
pub struct Command {
    dep1: String,
    dep2: String,
}

impl Run for Command {
    fn run(self) -> anyhow::Result<ExitCode> {
        let d1 = super::dep_new(&self.dep1)?;
        let d2 = super::dep_new(&self.dep2)?;
        Ok(ExitCode::from(!d1.intersects(&d2) as u8))
    }
}
