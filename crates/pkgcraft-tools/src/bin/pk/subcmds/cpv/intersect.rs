use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::{Cpv, CpvOrDep};
use pkgcraft::traits::Intersects;

#[derive(Debug, Args)]
pub(crate) struct Command {
    cpv: String,
    cpv_or_dep: String,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let cpv = Cpv::try_new(&self.cpv)?;
        let cpv_or_dep = CpvOrDep::try_new(&self.cpv_or_dep)?;
        Ok(ExitCode::from(!cpv.intersects(&cpv_or_dep) as u8))
    }
}
