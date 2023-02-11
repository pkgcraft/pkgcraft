use std::io::stdin;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::bail;
use clap::Args;
use is_terminal::IsTerminal;
use pkgcraft::atom::Version;

use crate::Run;

#[derive(Debug, Args)]
pub struct Sort {
    vals: Vec<String>,
}

impl Run for Sort {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut versions = Vec::<Version>::new();

        if self.vals.is_empty() || self.vals[0] == "-" {
            if stdin().is_terminal() {
                bail!("missing input on stdin");
            }

            for line in stdin().lines() {
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
        for v in versions {
            println!("{v}");
        }
        Ok(ExitCode::SUCCESS)
    }
}
