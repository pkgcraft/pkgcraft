use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::MaybeStdinVec;
use pkgcraft::dep::Version;
use strum::{Display, EnumIter, EnumString};

use crate::format::{EnumVariable, FormatString};

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Output using a custom format
    #[arg(short, long)]
    format: Option<String>,
    /// Values to parse
    values: Vec<MaybeStdinVec<String>>,
}

#[derive(Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
pub(crate) enum Key {
    OP,
    VER,
    REV,
}

impl EnumVariable<'_> for Key {
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

impl FormatString<'_> for Command {
    type Object = Version;
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
            if let Ok(ver) = Version::try_new(s) {
                if let Some(fmt) = &self.format {
                    writeln!(stdout, "{}", self.format_str(fmt, &ver)?)?;
                }
            } else {
                writeln!(stderr, "INVALID VERSION: {s}")?;
                failed = true;
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
