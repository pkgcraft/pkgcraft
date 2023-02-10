use std::io;
use std::process::ExitCode;
use std::str::FromStr;

use aho_corasick::AhoCorasick;
use anyhow::bail;
use clap::Args;
use is_terminal::IsTerminal;
use pkgcraft::atom::Atom;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Parse {
    /// Output using a custom format
    #[arg(short, long)]
    format: Option<String>,
    /// Atoms to parse, uses stdin if empty or "-"
    #[arg(value_name = "ATOM", required = false)]
    atoms: Vec<String>,
}

#[derive(Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
enum Key {
    CATEGORY,
    P,
    PF,
    PN,
    PR,
    PV,
    PVR,
    CPN,
    CPV,
    SLOT,
    SUBSLOT,
    REPO,
    ATOM,
}

impl Key {
    fn value(&self, atom: &Atom) -> String {
        use Key::*;
        match self {
            CATEGORY => atom.category().to_string(),
            P => atom.p(),
            PF => atom.pf(),
            PN => atom.package().to_string(),
            PR => atom.pr(),
            PV => atom.pv(),
            PVR => atom.pvr(),
            CPN => atom.cpn(),
            CPV => atom.cpv(),
            SLOT => atom.slot().unwrap_or_default().to_string(),
            SUBSLOT => atom.subslot().unwrap_or_default().to_string(),
            REPO => atom.repo().unwrap_or_default().to_string(),
            ATOM => atom.to_string(),
        }
    }
}

impl Parse {
    fn parse_atoms<I, S>(&self, iter: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for s in iter {
            let atom = Atom::from_str(s.as_ref())?;
            if let Some(format) = &self.format {
                let patterns: Vec<_> = Key::iter().map(|k| format!("{{{k}}}")).collect();
                let ac = AhoCorasick::new(patterns);
                let mut result = String::new();
                ac.replace_all_with(format, &mut result, |_mat, mat_str, dst| {
                    // strip match wrappers and convert to Key variant
                    let key_str = &mat_str[1..mat_str.len() - 1];
                    let key = Key::from_str(key_str)
                        .unwrap_or_else(|_| panic!("invalid pattern: {key_str}"));
                    // replace match with the related Atom value
                    dst.push_str(&key.value(&atom));
                    true
                });
                println!("{result}");
            }
        }
        Ok(())
    }
}

impl Run for Parse {
    fn run(&self) -> anyhow::Result<ExitCode> {
        if self.atoms.is_empty() || self.atoms[0] == "-" {
            if io::stdin().is_terminal() {
                bail!("missing input on stdin");
            }
            self.parse_atoms(io::stdin().lines().filter_map(|l| l.ok()))?;
        } else {
            self.parse_atoms(&self.atoms)?;
        };

        Ok(ExitCode::SUCCESS)
    }
}
