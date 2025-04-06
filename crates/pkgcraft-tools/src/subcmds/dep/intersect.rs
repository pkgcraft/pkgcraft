use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::{CpvOrDep, Dep};
use pkgcraft::traits::Intersects;

#[derive(Args)]
pub(crate) struct Command {
    dep: Dep,
    cpv_or_dep: CpvOrDep,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        Ok(ExitCode::from(!self.dep.intersects(&self.cpv_or_dep) as u8))
    }
}
