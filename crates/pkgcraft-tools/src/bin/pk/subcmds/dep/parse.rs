use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::MaybeStdinVec;
use pkgcraft::dep::Dep;
use pkgcraft::eapi::Eapi;
use strum::{Display, EnumIter, EnumString};

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
    /// Values to parse
    values: Vec<MaybeStdinVec<String>>,
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

impl EnumVariable<'_> for Key {
    type Object = Dep;

    fn value(&self, obj: &Self::Object) -> String {
        use Key::*;
        match self {
            BLOCKER => obj.blocker().map(|x| x.to_string()).unwrap_or_default(),
            CATEGORY => obj.category().to_string(),
            P => obj.cpv().map(|x| x.p()).unwrap_or_default(),
            PF => obj.cpv().map(|x| x.pf()).unwrap_or_default(),
            PN => obj.package().to_string(),
            PR => obj.cpv().map(|x| x.pr()).unwrap_or_default(),
            PV => obj.cpv().map(|x| x.pv()).unwrap_or_default(),
            PVR => obj.cpv().map(|x| x.pvr()).unwrap_or_default(),
            CPN => obj.cpn().to_string(),
            CPV => obj.cpv().map(|x| x.to_string()).unwrap_or_default(),
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

impl FormatString<'_> for Command {
    type Object = Dep;
    type FormatKey = Key;
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let eapi = self.eapi.unwrap_or_default();
        let mut failed = false;
        let (mut stdout, mut stderr) = (io::stdout().lock(), io::stderr().lock());

        for s in self
            .values
            .iter()
            .flatten()
            .flat_map(|s| s.split_whitespace())
        {
            if let Ok(dep) = eapi.dep(s) {
                if let Some(fmt) = &self.format {
                    writeln!(stdout, "{}", self.format_str(fmt, &dep)?)?;
                }
            } else {
                writeln!(stderr, "INVALID DEP: {s}")?;
                failed = true;
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
