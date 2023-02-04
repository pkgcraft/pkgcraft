use std::io;
use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use itertools::Itertools;
use pkgcraft::atom::Atom;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Sort {
    vals: Vec<String>,
}

impl Run for Sort {
    fn run(&self) -> anyhow::Result<ExitCode> {
        let mut atoms = Vec::<Atom>::new();

        if self.vals.len() == 1 && self.vals[0] == "-" {
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
