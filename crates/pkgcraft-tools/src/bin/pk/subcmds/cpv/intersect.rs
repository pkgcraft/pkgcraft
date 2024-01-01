use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Cpv;
use pkgcraft::traits::Intersects;

#[derive(Debug, Args)]
pub struct Command {
    value1: String,
    value2: String,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let cpv1 = Cpv::parse(&self.value1)?;
        let cpv2 = Cpv::parse(&self.value2)?;
        Ok(ExitCode::from(!cpv1.intersects(&cpv2) as u8))
    }
}
