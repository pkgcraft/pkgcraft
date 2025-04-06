use std::process::ExitCode;

use pkgcraft::dep::Version;
use pkgcraft::traits::Intersects;

#[derive(clap::Args)]
pub(crate) struct Command {
    ver1: Version,
    ver2: Version,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        Ok(ExitCode::from(!self.ver1.intersects(&self.ver2) as u8))
    }
}
