use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::MaybeStdinVec;
use pkgcraft::dep::Dep;
use pkgcraft::eapi::Eapi;
use strum::{Display, EnumIter, EnumString};

use crate::format::{EnumVariable, FormatString};

#[derive(Args)]
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
        match self {
            Self::BLOCKER => obj.blocker().map(|x| x.to_string()).unwrap_or_default(),
            Self::CATEGORY => obj.category().to_string(),
            Self::P => obj.cpv().map(|x| x.p()).unwrap_or_default(),
            Self::PF => obj.cpv().map(|x| x.pf()).unwrap_or_default(),
            Self::PN => obj.package().to_string(),
            Self::PR => obj.cpv().map(|x| x.pr()).unwrap_or_default(),
            Self::PV => obj.cpv().map(|x| x.pv()).unwrap_or_default(),
            Self::PVR => obj.cpv().map(|x| x.pvr()).unwrap_or_default(),
            Self::CPN => obj.cpn().to_string(),
            Self::CPV => obj.cpv().map(|x| x.to_string()).unwrap_or_default(),
            Self::OP => obj.op().map(|x| x.to_string()).unwrap_or_default(),
            Self::SLOT => obj.slot().unwrap_or_default().to_string(),
            Self::SUBSLOT => obj.subslot().unwrap_or_default().to_string(),
            Self::SLOT_OP => obj.slot_op().map(|x| x.to_string()).unwrap_or_default(),
            Self::REPO => obj.repo().unwrap_or_default().to_string(),
            Self::USE => obj
                .use_deps()
                .map(|x| x.iter().join(","))
                .unwrap_or_default(),
            Self::DEP => obj.to_string(),
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
