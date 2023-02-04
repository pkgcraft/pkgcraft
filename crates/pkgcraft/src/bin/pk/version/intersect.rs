use std::process::ExitCode;
use std::str::FromStr;

use anyhow::anyhow;
use clap::Args;
use itertools::Itertools;
use pkgcraft::atom::Version;

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
        let (v1, v2) = (Version::from_str(s1)?, Version::from_str(s2)?);
        Ok(ExitCode::from(!v1.intersects(&v2) as u8))
    }
}
