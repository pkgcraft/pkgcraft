use std::mem;
use std::process::ExitCode;

use anyhow::{anyhow, bail};
use clap::Args;
use itertools::Itertools;
use pkgcraft::dep::CpvOrDep;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub struct Command {
    /// Comparison expressions (uses stdin if "-")
    #[arg(value_name = "EXPR")]
    exprs: Vec<String>,
}

impl Command {
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;

        let exprs = mem::take(&mut self.exprs);
        for s in exprs.stdin_or_args() {
            let (lhs, op, rhs) = s
                .split_whitespace()
                .collect_tuple()
                .ok_or_else(|| anyhow!("invalid comparison format: {s}"))?;
            let lhs = CpvOrDep::parse(lhs)?;
            let rhs = CpvOrDep::parse(rhs)?;
            let result = match op {
                "<" => lhs < rhs,
                "<=" => lhs <= rhs,
                "==" => lhs == rhs,
                "!=" => lhs != rhs,
                ">=" => lhs >= rhs,
                ">" => lhs > rhs,
                _ => bail!("invalid operator: {op}"),
            };

            if !result {
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
