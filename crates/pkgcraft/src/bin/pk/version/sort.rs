use std::io;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::bail;
use clap::Args;
use is_terminal::IsTerminal;
use itertools::Itertools;
use pkgcraft::atom::Version;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Sort {
    vals: Vec<String>,
}

impl Run for Sort {
    fn run(&self) -> anyhow::Result<ExitCode> {
        let mut versions = Vec::<Version>::new();

        if self.vals.is_empty() || self.vals[0] == "-" {
            if io::stdin().is_terminal() {
                bail!("missing input on stdin");
            }

            for line in io::stdin().lines() {
                for s in line?.split_whitespace() {
                    versions.push(Version::from_str(s)?);
                }
            }
        } else {
            for s in &self.vals {
                versions.push(Version::from_str(s)?);
            }
        }

        versions.sort();
        println!("{}", versions.iter().map(|a| a.to_string()).join("\n"));
        Ok(ExitCode::SUCCESS)
    }
}
