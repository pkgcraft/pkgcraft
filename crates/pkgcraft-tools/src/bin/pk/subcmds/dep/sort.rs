use std::io::{stdin, stdout, Write};
use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::dep::Dep;

use crate::StdinArgs;

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, _config: &Config) -> anyhow::Result<ExitCode> {
        let mut deps = Vec::<Dep>::new();

        if self.vals.stdin_args()? {
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    deps.push(Dep::from_str(s)?);
                }
            }
        } else {
            for s in &self.vals {
                deps.push(Dep::from_str(s)?);
            }
        }

        deps.sort();
        for d in deps {
            writeln!(stdout(), "{d}")?;
        }
        Ok(ExitCode::SUCCESS)
    }
}
