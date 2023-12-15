use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Dep;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(self) -> anyhow::Result<ExitCode> {
        let deps: Vec<_> = self.vals.stdin_or_args().split_whitespace().collect();
        let deps: Result<Vec<_>, _> = deps
            .iter()
            .map(|s| Dep::parse(s, Default::default()))
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
