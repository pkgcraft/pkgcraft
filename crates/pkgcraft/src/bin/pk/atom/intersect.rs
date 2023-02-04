use std::process::ExitCode;
use std::str::FromStr;

use anyhow::anyhow;
use clap::Args;
use itertools::Itertools;
use pkgcraft::atom::Atom;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Intersect {
    vals: Vec<String>,
}

impl Run for Intersect {
    fn run(&self) -> anyhow::Result<ExitCode> {
        let (s1, s2) = self
            .vals
            .iter()
            .collect_tuple()
            .ok_or_else(|| anyhow!("invalid intersects args: {:?}", self.vals))?;
        let (a1, a2) = (Atom::from_str(s1)?, Atom::from_str(s2)?);
        Ok(ExitCode::from(!a1.intersects(&a2) as u8))
    }
}
