use std::io::stdin;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::bail;
use clap::Args;
use is_terminal::IsTerminal;
use pkgcraft::atom::Atom;
use strum::{Display, EnumIter, EnumString};

use crate::format::{EnumVariable, FormatString};
use crate::Run;

#[derive(Debug, Args)]
pub struct Parse {
    // options
    /// Use a specific EAPI
    #[arg(long)]
    eapi: Option<String>,
    /// Output using a custom format
    #[arg(short, long)]
    format: Option<String>,

    // positionals
    /// Atoms to parse, uses stdin if empty or "-"
    #[arg(value_name = "ATOM", required = false)]
    atoms: Vec<String>,
}

#[derive(Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
pub enum Key {
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

impl EnumVariable for Key {
    type Object = Atom;

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

impl FormatString for Parse {
    type Object = Atom;
    type FormatKey = Key;
}

impl Parse {
    fn parse_atom(&self, s: &str) -> anyhow::Result<()> {
        // parse atom, falling back to cpv if no EAPI was specified
        let atom = match &self.eapi {
            Some(eapi) => Atom::new(s, eapi.as_str()),
            None => Atom::from_str(s).or_else(|_| Atom::new_cpv(s)),
        }?;

        // output formatted string if specified
        if let Some(fmt) = &self.format {
            println!("{}", self.format(fmt, &atom));
        }

        Ok(())
    }
}

impl Run for Parse {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;
        // parse an atom, tracking overall process status
        let mut parse = |s: &str| {
            if self.parse_atom(s).is_err() {
                eprintln!("INVALID ATOM: {s}");
                status = ExitCode::FAILURE;
            }
        };

        if self.atoms.is_empty() || self.atoms[0] == "-" {
            if stdin().is_terminal() {
                bail!("missing input on stdin");
            }
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    parse(s);
                }
            }
        } else {
            for s in &self.atoms {
                parse(s);
            }
        }

        Ok(status)
    }
}
