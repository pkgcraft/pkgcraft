use std::io::stdin;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::bail;
use clap::Args;
use is_terminal::IsTerminal;
use pkgcraft::dep::Dep;

use crate::Run;

#[derive(Debug, Args)]
pub struct Sort {
    vals: Vec<String>,
}

impl Run for Sort {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut deps = Vec::<Dep>::new();

        if self.vals.is_empty() || self.vals[0] == "-" {
            if stdin().is_terminal() {
                bail!("missing input on stdin");
            }

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
            println!("{d}");
        }
        Ok(ExitCode::SUCCESS)
    }
}
