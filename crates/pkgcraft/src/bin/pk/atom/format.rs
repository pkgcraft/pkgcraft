use std::process::ExitCode;
use std::str::FromStr;

use aho_corasick::AhoCorasick;
use clap::Args;
use pkgcraft::atom::Atom;

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Format {
    format: String,
    atom: String,
}

impl Run for Format {
    fn run(&self) -> anyhow::Result<ExitCode> {
        let a = Atom::from_str(&self.atom)?;
        let (mut patterns, mut values) = (vec![], vec![]);
        let ver_default = a.version().map(|v| v.as_str()).unwrap_or_default();
        for (pat, val) in [
            ("{CATEGORY}", a.category().to_string()),
            ("{P}", format!("{}-{}", a.package(), ver_default)),
            ("{PN}", a.package().to_string()),
            ("{PV}", ver_default.to_string()),
        ] {
            patterns.push(pat);
            values.push(val);
        }

        let ac = AhoCorasick::new(&patterns);
        let result = ac.replace_all(&self.format, &values);
        println!("{result}");
        Ok(ExitCode::SUCCESS)
    }
}
