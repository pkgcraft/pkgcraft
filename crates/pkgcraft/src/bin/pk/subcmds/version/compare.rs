use std::process::ExitCode;

use anyhow::{anyhow, bail};
use clap::Args;
use itertools::Itertools;
use pkgcraft::dep::Version;

use crate::Run;

#[derive(Debug, Args)]
pub struct Command {
    compare: String,
}

impl Run for Command {
    fn run(self) -> anyhow::Result<ExitCode> {
        let (s1, op, s2) = self
            .compare
            .split_whitespace()
            .collect_tuple()
            .ok_or_else(|| anyhow!("invalid comparison format: {}", self.compare))?;
        let a1 = Version::new(s1)?;
        let a2 = Version::new(s2)?;
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
