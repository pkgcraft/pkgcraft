use std::process::ExitCode;

use pkgcraft::dep::Version;

use crate::Run;

#[derive(Debug, clap::Args)]
pub struct Command {
    ver1: String,
    ver2: String,
}

impl Run for Command {
    fn run(self) -> anyhow::Result<ExitCode> {
        let v1 = Version::new_optional_op(&self.ver1)?;
        let v2 = Version::new_optional_op(&self.ver2)?;
        Ok(ExitCode::from(!v1.intersects(&v2) as u8))
    }
}
