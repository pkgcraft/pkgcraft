use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::atom::Atom;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Intersect {
    atom1: String,
    atom2: String,
}

impl Run for Intersect {
    fn run(&self) -> anyhow::Result<ExitCode> {
        let a1 = Atom::from_str(&self.atom1)?;
        let a2 = Atom::from_str(&self.atom2)?;
        Ok(ExitCode::from(!a1.intersects(&a2) as u8))
    }
}
