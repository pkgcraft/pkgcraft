use std::process::ExitCode;

use crate::Run;

#[derive(Debug, clap::Args)]
pub struct Intersect {
    ver1: String,
    ver2: String,
}

impl Run for Intersect {
    fn run(self) -> anyhow::Result<ExitCode> {
        let v1 = super::ver_new(&self.ver1)?;
        let v2 = super::ver_new(&self.ver2)?;
        Ok(ExitCode::from(!v1.intersects(&v2) as u8))
    }
}
