use std::io;
use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
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

        if self.vals.len() == 1 && self.vals[0] == "-" {
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
