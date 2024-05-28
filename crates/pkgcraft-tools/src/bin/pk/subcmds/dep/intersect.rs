use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::{CpvOrDep, Dep};
use pkgcraft::traits::Intersects;

#[derive(Debug, Args)]
pub(crate) struct Command {
    dep: String,
    cpv_or_dep: String,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let dep = Dep::try_new(&self.dep)?;
        let cpv_or_dep = CpvOrDep::try_new(&self.cpv_or_dep)?;
        Ok(ExitCode::from(!dep.intersects(&cpv_or_dep) as u8))
    }
}
