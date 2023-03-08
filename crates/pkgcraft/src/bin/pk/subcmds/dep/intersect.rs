use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::{Dep, Intersects};

use crate::Run;

#[derive(Debug, Args)]
pub struct Command {
    dep1: String,
    dep2: String,
}

impl Run for Command {
    fn run(self) -> anyhow::Result<ExitCode> {
        let d1 = Dep::new_or_cpv(&self.dep1)?;
        let d2 = Dep::new_or_cpv(&self.dep2)?;
        Ok(ExitCode::from(!d1.intersects(&d2) as u8))
    }
}
