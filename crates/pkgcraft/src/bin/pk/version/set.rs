use std::io;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::bail;
use clap::Args;
use indexmap::IndexSet;
use is_terminal::IsTerminal;
use pkgcraft::atom::Version;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Set {
    vals: Vec<String>,
}

impl Run for Set {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut versions = IndexSet::<Version>::new();

        if self.vals.is_empty() || self.vals[0] == "-" {
            if io::stdin().is_terminal() {
                bail!("missing input on stdin");
            }

            for line in io::stdin().lines() {
                for s in line?.split_whitespace() {
                    versions.insert(Version::from_str(s)?);
                }
            }
        } else {
            for s in &self.vals {
                versions.insert(Version::from_str(s)?);
            }
        }

        for v in versions {
            println!("{v}");
        }
        Ok(ExitCode::SUCCESS)
    }
}
