use std::io::stdin;
use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use indexmap::IndexSet;
use pkgcraft::dep::Dep;

use crate::{Run, StdinArgs};

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Run for Command {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut deps = IndexSet::<Dep>::new();

        if self.vals.stdin_args()? {
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    deps.insert(Dep::from_str(s)?);
                }
            }
        } else {
            for s in &self.vals {
                deps.insert(Dep::from_str(s)?);
            }
        }

        for d in deps {
            println!("{d}");
        }
        Ok(ExitCode::SUCCESS)
    }
}
