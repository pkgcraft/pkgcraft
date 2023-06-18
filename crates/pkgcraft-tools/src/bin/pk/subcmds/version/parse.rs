use std::mem;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Version;
use strum::{Display, EnumIter, EnumString};

use crate::args::stdin_or_args;
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
            println!("{}", self.format_str(fmt, &ver)?);
        }
        Ok(())
    }
}

impl Command {
    pub(super) fn run(mut self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;

        for s in stdin_or_args(mem::take(&mut self.vals)) {
            if self.parse_version(&s).is_err() {
                eprintln!("INVALID VERSION: {s}");
                status = ExitCode::FAILURE;
            }
        }

        Ok(status)
    }
}
