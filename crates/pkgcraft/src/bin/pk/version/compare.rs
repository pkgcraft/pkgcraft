use std::process::ExitCode;
use std::str::FromStr;

use anyhow::{anyhow, bail};
use clap::Args;
use itertools::Itertools;
use pkgcraft::atom::Version;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Compare {
    compare: String,
}

impl Run for Compare {
    fn run(&self) -> anyhow::Result<ExitCode> {
        let (s1, op, s2) = self
            .compare
            .split_whitespace()
            .collect_tuple()
            .ok_or_else(|| anyhow!("invalid comparison format: {}", self.compare))?;
        let a1 = Version::from_str(s1)?;
        let a2 = Version::from_str(s2)?;
        let result = match op {
            "<" => a1 < a2,
            "<=" => a1 <= a2,
            "==" => a1 == a2,
            "!=" => a1 != a2,
            ">=" => a1 >= a2,
            ">" => a1 > a2,
            _ => bail!("invalid operator: {op}"),
        };
        Ok(ExitCode::from(!result as u8))
    }
}
