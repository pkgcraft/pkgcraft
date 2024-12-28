use std::process::ExitCode;

use anyhow::{anyhow, bail};
use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::MaybeStdinVec;
use pkgcraft::dep::Dep;

#[derive(Args)]
pub(crate) struct Command {
    /// Comparison expressions
    #[arg(
        value_name = "EXPR",
        long_help = indoc::indoc! {r#"
            Dep comparison expressions.

            Valid comparison expressions consist of two deps separated by
            whitespace with an operator between them. Supported operators
            include <, <=, ==, !=, >=, and >.

            For example, to test if one dep is less than or equal to another
            use: `pk dep compare "=cat/pkg-1.2.3-r1 <= =cat/pkg-1.2.3-r2"` which
            returns shell true (0) when run.

            Expressions are read from standard input if `-` is used."#
        }
    )]
    values: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let mut success = true;

        for s in self.values.iter().flatten() {
            let (lhs, op, rhs) = s
                .split_whitespace()
                .collect_tuple()
                .ok_or_else(|| anyhow!("invalid comparison format: {s}"))?;

            let lhs = Dep::try_new(lhs)?;
            let rhs = Dep::try_new(rhs)?;

            success &= match op {
                "<" => lhs < rhs,
                "<=" => lhs <= rhs,
                "==" => lhs == rhs,
                "!=" => lhs != rhs,
                ">=" => lhs >= rhs,
                ">" => lhs > rhs,
                _ => bail!("invalid operator: {s}"),
            };
        }

        Ok(ExitCode::from(!success as u8))
    }
}
