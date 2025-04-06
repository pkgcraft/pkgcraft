use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::{Cpv, CpvOrDep};
use pkgcraft::traits::Intersects;

#[derive(Args)]
pub(crate) struct Command {
    cpv: Cpv,
    cpv_or_dep: CpvOrDep,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        Ok(ExitCode::from(!self.cpv.intersects(&self.cpv_or_dep) as u8))
    }
}
