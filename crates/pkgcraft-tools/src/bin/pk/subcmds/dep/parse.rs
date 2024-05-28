use std::io::{self, Write};
use std::mem;
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::dep::Dep;
use pkgcraft::eapi::Eapi;
use strum::{Display, EnumIter, EnumString};

use crate::args::StdinOrArgs;
use crate::format::{EnumVariable, FormatString};

#[derive(Debug, Args)]
pub(crate) struct Command {
    // options
    /// Use a specific EAPI
    #[arg(long)]
    eapi: Option<&'static Eapi>,
    /// Output using a custom format
    #[arg(short, long)]
    format: Option<String>,

    // positionals
    /// Values to parse (uses stdin if "-")
    values: Vec<String>,
}

#[derive(Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
pub(crate) enum Key {
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
    USE,
    DEP,
}

impl<'a> EnumVariable<'a> for Key {
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
            CPN => obj.cpn().to_string(),
            CPV => obj.cpv(),
            OP => obj.op().map(|x| x.to_string()).unwrap_or_default(),
            SLOT => obj.slot().unwrap_or_default().to_string(),
            SUBSLOT => obj.subslot().unwrap_or_default().to_string(),
            SLOT_OP => obj.slot_op().map(|x| x.to_string()).unwrap_or_default(),
            REPO => obj.repo().unwrap_or_default().to_string(),
            USE => obj
                .use_deps()
                .map(|x| x.iter().join(","))
                .unwrap_or_default(),
            DEP => obj.to_string(),
        }
    }
}

impl<'a> FormatString<'a> for Command {
    type Object = Dep;
    type FormatKey = Key;
}

impl Command {
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let eapi = self.eapi.unwrap_or_default();
        let mut status = ExitCode::SUCCESS;
        let (mut stdout, mut stderr) = (io::stdout().lock(), io::stderr().lock());

        let values = mem::take(&mut self.values);
        for s in values.stdin_or_args().split_whitespace() {
            if let Ok(dep) = eapi.dep(&s) {
                if let Some(fmt) = &self.format {
                    writeln!(stdout, "{}", self.format_str(fmt, &dep)?)?;
                }
            } else {
                writeln!(stderr, "INVALID DEP: {s}")?;
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
