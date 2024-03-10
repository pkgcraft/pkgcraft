use std::mem;
use std::process::ExitCode;

use anyhow::{anyhow, bail};
use clap::Args;
use itertools::Itertools;
use pkgcraft::dep::Version;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Version comparison expressions (uses stdin if "-")
    #[arg(
        value_name = "EXPR",
        long_help = indoc::indoc! {r#"
            Version comparison expressions.

            These consist of two versions separated by a whitespace with a
            version operator between them. Supported operators include <, <=,
            ==, !=, >=, and >.

            For example, to test if 1.2.3-r1 is less than or equal to 1.2.3-r2
            use: `pk version compare "1.2.3-r1 <= 1.2.3-r2"` which returns shell
            true (0) when run."#
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

            let lhs = Version::parse(lhs)?;
            let rhs = Version::parse(rhs)?;

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
