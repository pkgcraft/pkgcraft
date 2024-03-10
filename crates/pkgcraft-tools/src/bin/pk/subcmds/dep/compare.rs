use std::mem;
use std::process::ExitCode;

use anyhow::{anyhow, bail};
use clap::Args;
use itertools::Itertools;
use pkgcraft::dep::Dep;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Comparison expressions (uses stdin if "-")
    #[arg(
        value_name = "EXPR",
        long_help = indoc::indoc! {r#"
            Dep comparison expressions.

            Valid comparison expressions consist of two deps separated by
            whitespace with an operator between them. Supported operators
            include <, <=, ==, !=, >=, and >.

            For example, to test if one dep is less than or equal to another
            use: `pk dep compare "=cat/pkg-1.2.3-r1 <= =cat/pkg-1.2.3-r2"` which
            returns shell true (0) when run."#
        }
    )]
    values: Vec<String>,
}

impl Command {
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;

        let values = mem::take(&mut self.values);
        for s in values.stdin_or_args() {
            let (lhs, op, rhs) = s
                .split_whitespace()
                .collect_tuple()
                .ok_or_else(|| anyhow!("invalid comparison format: {s}"))?;

            let lhs = Dep::parse(lhs, Default::default())?;
            let rhs = Dep::parse(rhs, Default::default())?;

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
