use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::MaybeStdinVec;
use pkgcraft::dep::Cpv;
use strum::{Display, EnumIter, EnumString};

use crate::format::{EnumVariable, FormatString};

#[derive(Debug, Args)]
pub(crate) struct Command {
    // options
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
    CATEGORY,
    P,
    PF,
    PN,
    PR,
    PV,
    PVR,
    CPN,
    CPV,
}

impl EnumVariable<'_> for Key {
    type Object = Cpv;

    fn value(&self, obj: &Self::Object) -> String {
        match self {
            Self::CATEGORY => obj.category().to_string(),
            Self::P => obj.p(),
            Self::PF => obj.pf(),
            Self::PN => obj.package().to_string(),
            Self::PR => obj.pr(),
            Self::PV => obj.pv(),
            Self::PVR => obj.pvr(),
            Self::CPN => obj.cpn().to_string(),
            Self::CPV => obj.to_string(),
        }
    }
}

impl FormatString<'_> for Command {
    type Object = Cpv;
    type FormatKey = Key;
}

impl Command {
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let mut failed = false;
        let (mut stdout, mut stderr) = (io::stdout().lock(), io::stderr().lock());

        for s in self
            .values
            .iter()
            .flatten()
            .flat_map(|s| s.split_whitespace())
        {
            if let Ok(cpv) = Cpv::try_new(s) {
                if let Some(fmt) = &self.format {
                    writeln!(stdout, "{}", self.format_str(fmt, &cpv)?)?;
                }
            } else {
                writeln!(stderr, "INVALID CPV: {s}")?;
                failed = true;
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
