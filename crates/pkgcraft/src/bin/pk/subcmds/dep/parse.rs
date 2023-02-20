use std::io::stdin;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Dep;
use strum::{Display, EnumIter, EnumString};

use crate::format::{EnumVariable, FormatString};
use crate::{Run, StdinArgs};

#[derive(Debug, Args)]
pub struct Command {
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

    fn value(&self, obj: &Self::Object) -> String {
        use Key::*;
        match self {
            BLOCKER => obj.blocker().map(|x| x.to_string()).unwrap_or_default(),
            CATEGORY => obj.category().to_string(),
            P => obj.p(),
            PF => obj.pf(),
            PN => obj.package().to_string(),
            PR => obj.pr(),
            PV => obj.pv(),
            PVR => obj.pvr(),
            CPN => obj.cpn(),
            CPV => obj.cpv(),
            OP => obj.op().map(|x| x.to_string()).unwrap_or_default(),
            SLOT => obj.slot().unwrap_or_default().to_string(),
            SUBSLOT => obj.subslot().unwrap_or_default().to_string(),
            SLOT_OP => obj.slot_op().map(|x| x.to_string()).unwrap_or_default(),
            REPO => obj.repo().unwrap_or_default().to_string(),
            DEP => obj.to_string(),
        }
    }
}

impl FormatString for Command {
    type Object = Dep;
    type FormatKey = Key;
}

impl Command {
    fn parse_dep(&self, s: &str) -> anyhow::Result<()> {
        let dep = match &self.eapi {
            Some(eapi) => Dep::new(s, eapi.as_str()),
            None => Dep::new_or_cpv(s),
        }?;

        // output formatted string if specified
        if let Some(fmt) = &self.format {
            println!("{}", self.format(fmt, &dep));
        }

        Ok(())
    }
}

impl Run for Command {
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
