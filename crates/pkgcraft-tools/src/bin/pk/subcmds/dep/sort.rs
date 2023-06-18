use std::io::{self, Write};
use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::dep::Dep;

use crate::args::stdin_or_args;

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let deps: Result<Vec<_>, _> = stdin_or_args(self.vals)
            .map(|s| Dep::from_str(&s))
            .collect();

        let mut deps = deps?;
        deps.sort();

        let mut handle = io::stdout().lock();
        for d in deps {
            writeln!(handle, "{d}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
