use std::io::{self, Write};
use std::mem;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Version;
use strum::{Display, EnumIter, EnumString};

use crate::args::StdinOrArgs;
use crate::format::{EnumVariable, FormatString};

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Output using a custom format
    #[arg(short, long)]
    format: Option<String>,
    /// Values to parse (uses stdin if "-")
    values: Vec<String>,
}

#[derive(Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
pub(crate) enum Key {
    OP,
    VER,
    REV,
}

impl<'a> EnumVariable<'a> for Key {
    type Object = Version;

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
    type Object = Version;
    type FormatKey = Key;
}

impl Command {
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;
        let (mut stdout, mut stderr) = (io::stdout().lock(), io::stderr().lock());

        let values = mem::take(&mut self.values);
        for s in values.stdin_or_args().split_whitespace() {
            if let Ok(ver) = Version::try_new(&s) {
                if let Some(fmt) = &self.format {
                    writeln!(stdout, "{}", self.format_str(fmt, &ver)?)?;
                }
            } else {
                writeln!(stderr, "INVALID VERSION: {s}")?;
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
