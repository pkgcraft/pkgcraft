use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::dep::{CpvOrDep, Intersects};

use crate::Run;

#[derive(Debug, Args)]
pub struct Command {
    dep1: String,
    dep2: String,
}

impl Run for Command {
    fn run(self, _config: &Config) -> anyhow::Result<ExitCode> {
        let obj1 = CpvOrDep::from_str(&self.dep1)?;
        let obj2 = CpvOrDep::from_str(&self.dep2)?;
        Ok(ExitCode::from(!obj1.intersects(&obj2) as u8))
    }
}
