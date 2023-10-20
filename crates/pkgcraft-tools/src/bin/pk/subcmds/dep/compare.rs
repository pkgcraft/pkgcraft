use std::mem;
use std::process::ExitCode;

use anyhow::{anyhow, bail};
use clap::Args;
use itertools::Itertools;
use pkgcraft::dep::Dep;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub struct Command {
    /// Dep comparison expressions (uses stdin if "-")
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
            let d1: Dep = s1.parse()?;
            let d2: Dep = s2.parse()?;
            let result = match op {
                "<" => d1 < d2,
                "<=" => d1 <= d2,
                "==" => d1 == d2,
                "!=" => d1 != d2,
                ">=" => d1 >= d2,
                ">" => d1 > d2,
                _ => bail!("invalid operator: {op}"),
            };

            if !result {
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
