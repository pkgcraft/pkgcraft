use std::io;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::bail;
use clap::Args;
use indexmap::IndexSet;
use is_terminal::IsTerminal;
use pkgcraft::atom::Atom;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Set {
    vals: Vec<String>,
}

impl Run for Set {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut atoms = IndexSet::<Atom>::new();

        if self.vals.is_empty() || self.vals[0] == "-" {
            if io::stdin().is_terminal() {
                bail!("missing input on stdin");
            }

            for line in io::stdin().lines() {
                for s in line?.split_whitespace() {
                    atoms.insert(Atom::from_str(s)?);
                }
            }
        } else {
            for s in &self.vals {
                atoms.insert(Atom::from_str(s)?);
            }
        }

        for a in atoms {
            println!("{a}");
        }
        Ok(ExitCode::SUCCESS)
    }
}
