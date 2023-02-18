use std::io::stdin;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Version;

use crate::{Run, StdinArgs};

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Run for Command {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut versions = Vec::<Version>::new();

        if self.vals.stdin_args()? {
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    versions.push(Version::new(s)?);
                }
            }
        } else {
            for s in &self.vals {
                versions.push(Version::new(s)?);
            }
        }

        versions.sort();
        for v in versions {
            println!("{v}");
        }
        Ok(ExitCode::SUCCESS)
    }
}
