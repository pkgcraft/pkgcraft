use std::io::stdin;
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
#[allow(non_camel_case_types)]
enum Key {
    BLOCKER,
    CATEGORY,
    P,
    PF,
    PN,
    PR,
    PV,
    PVR,
    CPN,
    CPV,
    OP,
    SLOT,
    SUBSLOT,
    SLOT_OP,
    REPO,
    ATOM,
}

impl Key {
    fn value(&self, atom: &Atom) -> String {
        use Key::*;
        match self {
            BLOCKER => atom.blocker().map(|x| x.to_string()).unwrap_or_default(),
            CATEGORY => atom.category().to_string(),
            P => atom.p(),
            PF => atom.pf(),
            PN => atom.package().to_string(),
            PR => atom.pr(),
            PV => atom.pv(),
            PVR => atom.pvr(),
            CPN => atom.cpn(),
            CPV => atom.cpv(),
            OP => atom.op().map(|x| x.to_string()).unwrap_or_default(),
            SLOT => atom.slot().unwrap_or_default().to_string(),
            SUBSLOT => atom.subslot().unwrap_or_default().to_string(),
            SLOT_OP => atom.slot_op().map(|x| x.to_string()).unwrap_or_default(),
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
                let patterns: Vec<_> = Key::iter()
                    .flat_map(|k| [format!("{{{k}}}"), format!("[{k}]")])
                    .collect();
                let ac = AhoCorasick::new(patterns);
                let mut result = String::new();
                ac.replace_all_with(format, &mut result, |_mat, mat_str, dst| {
                    // strip match wrappers and convert to Key variant
                    let mat_type = &mat_str[0..1];
                    let key_str = &mat_str[1..mat_str.len() - 1];
                    let key = Key::from_str(key_str)
                        .unwrap_or_else(|_| panic!("invalid pattern: {key_str}"));

                    // replace match with the related value
                    match key.value(&atom).as_str() {
                        "" if mat_type == "{" => dst.push_str("<unset>"),
                        s => dst.push_str(s),
                    }

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
            if stdin().is_terminal() {
                bail!("missing input on stdin");
            }
            self.parse_atoms(stdin().lines().filter_map(|l| l.ok()))?;
        } else {
            self.parse_atoms(&self.atoms)?;
        };

        Ok(ExitCode::SUCCESS)
    }
}
