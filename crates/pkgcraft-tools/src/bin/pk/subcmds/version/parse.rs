use std::io::{stderr, stdin, stdout, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::dep::Version;
use strum::{Display, EnumIter, EnumString};

use crate::format::{EnumVariable, FormatString};
use crate::StdinArgs;

#[derive(Debug, Args)]
pub struct Command {
    /// Output using a custom format
    #[arg(short, long)]
    format: Option<String>,
    /// Versions to parse, uses stdin if empty or "-"
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

impl EnumVariable for Key {
    type Object = Version;

    fn value(&self, obj: &Self::Object) -> String {
        use Key::*;
        match self {
            OP => obj.op().map(|x| x.to_string()).unwrap_or_default(),
            VER => obj.as_str().to_string(),
            REV => obj
                .revision()
                .map(|r| r.as_str())
                .unwrap_or_default()
                .to_string(),
        }
    }
}

impl FormatString for Command {
    type Object = Version;
    type FormatKey = Key;
}

impl Command {
    fn parse_version(&self, s: &str) -> anyhow::Result<()> {
        let ver = Version::new(s)?;
        if let Some(fmt) = &self.format {
            writeln!(stdout(), "{}", self.format_str(fmt, &ver)?)?;
        }
        Ok(())
    }
}

impl Command {
    pub(super) fn run(&self, _config: &Config) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;
        // parse a version, tracking overall process status
        let mut parse = |s: &str| -> anyhow::Result<()> {
            if self.parse_version(s).is_err() {
                writeln!(stderr(), "INVALID VERSION: {s}")?;
                status = ExitCode::FAILURE;
            }
            Ok(())
        };

        if self.vals.stdin_args()? {
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    parse(s)?;
                }
            }
        } else {
            for s in &self.vals {
                parse(s)?;
            }
        }

        Ok(status)
    }
}
