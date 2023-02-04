use std::process::ExitCode;
use std::str::FromStr;

use anyhow::anyhow;
use itertools::Itertools;
use pkgcraft::atom::Version;

use crate::Run;

#[derive(Debug, clap::Args)]
pub(crate) struct Args {
    vals: Vec<String>,
}

impl Run for Args {
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
