use std::process::ExitCode;
use std::str::FromStr;

use aho_corasick::AhoCorasick;
use clap::Args;
use pkgcraft::atom::Version;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Format {
    format: String,
    version: String,
}

impl Run for Format {
    fn run(self) -> anyhow::Result<ExitCode> {
        let v = Version::from_str(&self.version)?;
        let (mut patterns, mut values) = (vec![], vec![]);
        for (pat, val) in
            [("{PV}", v.as_str()), ("{REV}", v.revision().map(|r| r.as_str()).unwrap_or_default())]
        {
            patterns.push(pat);
            values.push(val);
        }

        let ac = AhoCorasick::new(&patterns);
        let result = ac.replace_all(&self.format, &values);
        println!("{result}");
        Ok(ExitCode::SUCCESS)
    }
}
