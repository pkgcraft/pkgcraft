use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::dep::Dep;

use crate::Run;

#[derive(Debug, Args)]
pub struct Intersect {
    dep1: String,
    dep2: String,
}

impl Run for Intersect {
    fn run(self) -> anyhow::Result<ExitCode> {
        let d1 = Dep::from_str(&self.dep1).or_else(|_| Dep::new_cpv(&self.dep1))?;
        let d2 = Dep::from_str(&self.dep2).or_else(|_| Dep::new_cpv(&self.dep2))?;
        Ok(ExitCode::from(!d1.intersects(&d2) as u8))
    }
}
