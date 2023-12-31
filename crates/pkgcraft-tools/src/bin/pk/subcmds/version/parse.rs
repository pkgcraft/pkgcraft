use std::mem;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Version;
use strum::{Display, EnumIter, EnumString};

use crate::args::StdinOrArgs;
use crate::format::{EnumVariable, FormatString};

#[derive(Debug, Args)]
pub struct Command {
    /// Output using a custom format
    #[arg(short, long)]
    format: Option<String>,
    /// Versions to parse (uses stdin if "-")
    #[arg(value_name = "VERSION", required = false)]
    vals: Vec<String>,
}

#[derive(Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
pub enum Key {
    OP,
    VER,
    REV,
}

impl<'a> EnumVariable<'a> for Key {
    type Object = Version<&'a str>;

    fn value(&self, obj: &Self::Object) -> String {
        use Key::*;
        match self {
            OP => obj.op().map(|x| x.to_string()).unwrap_or_default(),
            VER => obj.without_op(),
            REV => obj
                .revision()
                .map(|r| r.as_str())
                .unwrap_or_default()
                .to_string(),
        }
    }
}

impl<'a> FormatString<'a> for Command {
    type Object = Version<&'a str>;
    type FormatKey = Key;
}

impl Command {
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;

        let vals = mem::take(&mut self.vals);
        for s in vals.stdin_or_args().split_whitespace() {
            if let Ok(ver) = Version::parse(&s) {
                if let Some(fmt) = &self.format {
                    println!("{}", self.format_str(fmt, &ver)?);
                }
            } else {
                eprintln!("INVALID VERSION: {s}");
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
