use std::io::{stdin, stdout, Write};
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexSet;
use pkgcraft::config::Config;
use pkgcraft::dep::Version;

use crate::StdinArgs;

#[derive(Debug, Args)]
pub struct Command {
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, _config: &Config) -> anyhow::Result<ExitCode> {
        let mut versions = IndexSet::<Version>::new();

        if self.vals.stdin_args()? {
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    versions.insert(Version::new(s)?);
                }
            }
        } else {
            for s in &self.vals {
                versions.insert(Version::new(s)?);
            }
        }

        for v in versions {
            writeln!(stdout(), "{v}")?;
        }
        Ok(ExitCode::SUCCESS)
    }
}
