use std::mem;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Cpv;
use strum::{Display, EnumIter, EnumString};

use crate::args::StdinOrArgs;
use crate::format::{EnumVariable, FormatString};

#[derive(Debug, Args)]
pub struct Command {
    // options
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
pub enum Key {
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

impl<'a> EnumVariable<'a> for Key {
    type Object = Cpv<&'a str>;

    fn value(&self, obj: &Self::Object) -> String {
        use Key::*;
        match self {
            CATEGORY => obj.category().to_string(),
            P => obj.p(),
            PF => obj.pf(),
            PN => obj.package().to_string(),
            PR => obj.pr(),
            PV => obj.pv(),
            PVR => obj.pvr(),
            CPN => obj.cpn(),
            CPV => obj.to_string(),
        }
    }
}

impl<'a> FormatString<'a> for Command {
    type Object = Cpv<&'a str>;
    type FormatKey = Key;
}

impl Command {
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;

        let values = mem::take(&mut self.values);
        for s in values.stdin_or_args().split_whitespace() {
            if let Ok(cpv) = Cpv::parse(&s) {
                if let Some(fmt) = &self.format {
                    println!("{}", self.format_str(fmt, &cpv)?);
                }
            } else {
                eprintln!("INVALID CPV: {s}");
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
