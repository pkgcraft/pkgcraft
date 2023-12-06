use std::process::ExitCode;

use pkgcraft::dep::Version;
use pkgcraft::traits::Intersects;

#[derive(Debug, clap::Args)]
pub struct Command {
    ver1: String,
    ver2: String,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let v1 = Version::new(&self.ver1)?;
        let v2 = Version::new(&self.ver2)?;
        Ok(ExitCode::from(!v1.intersects(&v2) as u8))
    }
}
