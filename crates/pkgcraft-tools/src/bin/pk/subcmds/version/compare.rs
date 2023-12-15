use std::mem;
use std::process::ExitCode;

use anyhow::{anyhow, bail};
use clap::Args;
use itertools::Itertools;
use pkgcraft::dep::Version;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub struct Command {
    /// Version comparison expressions (uses stdin if "-")
    #[arg(value_name = "EXPR")]
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;

        let vals = mem::take(&mut self.vals);
        for s in vals.stdin_or_args() {
            let (s1, op, s2) = s
                .split_whitespace()
                .collect_tuple()
                .ok_or_else(|| anyhow!("invalid comparison format: {s}"))?;
            let a1 = Version::parse(s1)?;
            let a2 = Version::parse(s2)?;
            let result = match op {
                "<" => a1 < a2,
                "<=" => a1 <= a2,
                "==" => a1 == a2,
                "!=" => a1 != a2,
                ">=" => a1 >= a2,
                ">" => a1 > a2,
                _ => bail!("invalid operator: {op}"),
            };

            if !result {
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
