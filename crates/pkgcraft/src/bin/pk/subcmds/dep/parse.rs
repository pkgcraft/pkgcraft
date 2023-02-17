use std::io::stdin;
use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::dep::Dep;
use strum::{Display, EnumIter, EnumString};

use crate::format::{EnumVariable, FormatString};
use crate::{Run, StdinArgs};

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
    /// Deps to parse, uses stdin if empty or "-"
    #[arg(value_name = "DEP", required = false)]
    vals: Vec<String>,
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
    DEP,
}

impl EnumVariable for Key {
    type Object = Dep;

    fn value(&self, dep: &Dep) -> String {
        use Key::*;
        match self {
            BLOCKER => dep.blocker().map(|x| x.to_string()).unwrap_or_default(),
            CATEGORY => dep.category().to_string(),
            P => dep.p(),
            PF => dep.pf(),
            PN => dep.package().to_string(),
            PR => dep.pr(),
            PV => dep.pv(),
            PVR => dep.pvr(),
            CPN => dep.cpn(),
            CPV => dep.cpv(),
            OP => dep.op().map(|x| x.to_string()).unwrap_or_default(),
            SLOT => dep.slot().unwrap_or_default().to_string(),
            SUBSLOT => dep.subslot().unwrap_or_default().to_string(),
            SLOT_OP => dep.slot_op().map(|x| x.to_string()).unwrap_or_default(),
            REPO => dep.repo().unwrap_or_default().to_string(),
            DEP => dep.to_string(),
        }
    }
}

impl FormatString for Parse {
    type Object = Dep;
    type FormatKey = Key;
}

impl Parse {
    fn parse_dep(&self, s: &str) -> anyhow::Result<()> {
        // parse dep, falling back to cpv if no EAPI was specified
        let dep = match &self.eapi {
            Some(eapi) => Dep::new(s, eapi.as_str()),
            None => Dep::from_str(s).or_else(|_| Dep::new_cpv(s)),
        }?;

        // output formatted string if specified
        if let Some(fmt) = &self.format {
            println!("{}", self.format(fmt, &dep));
        }

        Ok(())
    }
}

impl Run for Parse {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;
        // parse a dep, tracking overall process status
        let mut parse = |s: &str| {
            if self.parse_dep(s).is_err() {
                eprintln!("INVALID DEP: {s}");
                status = ExitCode::FAILURE;
            }
        };

        if self.vals.stdin_args()? {
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    parse(s);
                }
            }
        } else {
            for s in &self.vals {
                parse(s);
            }
        }

        Ok(status)
    }
}
