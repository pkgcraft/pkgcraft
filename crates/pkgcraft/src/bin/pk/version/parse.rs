use std::io::stdin;
use std::process::ExitCode;
use std::str::FromStr;

use aho_corasick::AhoCorasick;
use anyhow::bail;
use clap::Args;
use is_terminal::IsTerminal;
use pkgcraft::atom::Version;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Parse {
    /// Output using a custom format
    #[arg(short, long)]
    format: Option<String>,
    /// Versions to parse, uses stdin if empty or "-"
    #[arg(value_name = "VERSION", required = false)]
    versions: Vec<String>,
}

#[derive(Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
enum Key {
    VER,
    REV,
}

impl Key {
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

impl Parse {
    fn parse_version(&self, s: &str) -> anyhow::Result<()> {
        let ver = Version::new(s).or_else(|_| Version::new_with_op(s))?;
        if let Some(format) = &self.format {
            let patterns: Vec<_> = Key::iter()
                .flat_map(|k| [format!("{{{k}}}"), format!("[{k}]")])
                .collect();
            let ac = AhoCorasick::new(patterns);
            let mut result = String::new();
            ac.replace_all_with(format, &mut result, |_mat, mat_str, dst| {
                // strip match wrappers and convert to Key variant
                let mat_type = &mat_str[0..1];
                let key_str = &mat_str[1..mat_str.len() - 1];
                let key =
                    Key::from_str(key_str).unwrap_or_else(|_| panic!("invalid pattern: {key_str}"));

                // replace match with the related value
                match key.value(&ver).as_str() {
                    "" if mat_type == "{" => dst.push_str("<unset>"),
                    s => dst.push_str(s),
                }

                true
            });
            println!("{result}");
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

        if self.versions.is_empty() || self.versions[0] == "-" {
            if stdin().is_terminal() {
                bail!("missing input on stdin");
            }
            for l in stdin().lines().filter_map(|l| l.ok()) {
                for s in l.split_whitespace() {
                    parse(s);
                }
            }
        } else {
            for s in &self.versions {
                parse(s);
            }
        }

        Ok(status)
    }
}
