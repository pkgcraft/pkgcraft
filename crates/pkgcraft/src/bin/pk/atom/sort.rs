use std::io;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::bail;
use clap::Args;
use is_terminal::IsTerminal;
use itertools::Itertools;
use pkgcraft::atom::Atom;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Sort {
    vals: Vec<String>,
}

impl Run for Sort {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut atoms = Vec::<Atom>::new();

        if self.vals.is_empty() || self.vals[0] == "-" {
            if io::stdin().is_terminal() {
                bail!("missing input on stdin");
            }

            for line in io::stdin().lines() {
                for s in line?.split_whitespace() {
                    atoms.push(Atom::from_str(s)?);
                }
            }
        } else {
            for s in &self.vals {
                atoms.push(Atom::from_str(s)?);
            }
        }

        atoms.sort();
        println!("{}", atoms.iter().map(|a| a.to_string()).join("\n"));
        Ok(ExitCode::SUCCESS)
    }
}
