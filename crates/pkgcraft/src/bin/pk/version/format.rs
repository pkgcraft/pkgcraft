use std::process::ExitCode;
use std::str::FromStr;

use aho_corasick::AhoCorasick;
use anyhow::anyhow;
use clap::Args;
use itertools::Itertools;
use pkgcraft::atom::Version;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Format {
    vals: Vec<String>,
}

impl Run for Format {
    fn run(&self) -> anyhow::Result<ExitCode> {
        let (haystack, s) = self
            .vals
            .iter()
            .collect_tuple()
            .ok_or_else(|| anyhow!("invalid format args: {:?}", self.vals))?;
        let v = Version::from_str(s)?;
        let (mut patterns, mut values) = (vec![], vec![]);
        for (pat, val) in [("{PV}", v.as_str()), ("{REV}", v.revision().as_str())] {
            patterns.push(pat);
            values.push(val);
        }

        let ac = AhoCorasick::new(&patterns);
        let result = ac.replace_all(haystack, &values);
        println!("{result}");
        Ok(ExitCode::SUCCESS)
    }
}
