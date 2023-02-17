use std::io::stdin;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::dep::Version;
use strum::{Display, EnumIter, EnumString};

use crate::format::{EnumVariable, FormatString};
use crate::{Run, StdinArgs};

#[derive(Debug, Args)]
pub struct Parse {
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
    VER,
    REV,
}

impl EnumVariable for Key {
    type Object = Version;

    fn value(&self, ver: &Version) -> String {
        use Key::*;
        match self {
            VER => ver.as_str().to_string(),
            REV => ver
                .revision()
                .map(|r| r.as_str())
                .unwrap_or_default()
                .to_string(),
        }
    }
}

impl FormatString for Parse {
    type Object = Version;
    type FormatKey = Key;
}

impl Parse {
    fn parse_version(&self, s: &str) -> anyhow::Result<()> {
        let ver = Version::new(s).or_else(|_| Version::new_with_op(s))?;
        if let Some(fmt) = &self.format {
            println!("{}", self.format(fmt, &ver));
        }
        Ok(())
    }
}

impl Run for Parse {
    fn run(self) -> anyhow::Result<ExitCode> {
        let mut status = ExitCode::SUCCESS;
        // parse a version, tracking overall process status
        let mut parse = |s: &str| {
            if self.parse_version(s).is_err() {
                eprintln!("INVALID VERSION: {s}");
                status = ExitCode::FAILURE;
            }
        };

        if self.vals.stdin_args()? {
            for line in stdin().lines() {
                for s in line?.split_whitespace() {
                    parse(s);
                }
            }
        } else {
            for s in &self.vals {
                parse(s);
            }
        }

        Ok(status)
    }
}
