use std::collections::HashMap;
use std::process::ExitCode;
use std::str::FromStr;

use aho_corasick::AhoCorasick;
use clap::Args;
use pkgcraft::atom::Atom;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::Run;

#[derive(Debug, Args)]
pub(crate) struct Format {
    format: String,
    atom: String,
}

#[derive(Display, EnumIter, EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum Key {
    CATEGORY,
    P,
    PF,
    PN,
    PR,
    PV,
    PVR,
    CPN,
    CPV,
    SLOT,
    SUBSLOT,
    REPO,
}

impl Key {
    fn value(&self, atom: &Atom) -> String {
        use Key::*;
        match self {
            CATEGORY => atom.category().to_string(),
            P => atom.p(),
            PF => atom.pf(),
            PN => atom.package().to_string(),
            PR => atom.pr(),
            PV => atom.pv(),
            PVR => atom.pvr(),
            CPN => atom.cpn(),
            CPV => atom.cpv(),
            SLOT => atom.slot().unwrap_or_default().to_string(),
            SUBSLOT => atom.subslot().unwrap_or_default().to_string(),
            REPO => atom.repo().unwrap_or_default().to_string(),
        }
    }
}

impl Run for Format {
    fn run(&self) -> anyhow::Result<ExitCode> {
        let atom = Atom::from_str(&self.atom)?;
        let patterns: Vec<_> = Key::iter().map(|k| format!("{{{k}}}")).collect();
        let mut key_cache = HashMap::<Key, String>::new();

        let ac = AhoCorasick::new(&patterns);
        let mut result = String::new();
        ac.replace_all_with(&self.format, &mut result, |_mat, s, dst| {
            // strip match wrappers and convert to Key variant
            let key_str = &s[1..s.len() - 1];
            let key =
                Key::from_str(key_str).unwrap_or_else(|_| panic!("invalid pattern: {key_str}"));

            // use cached key values to avoid redundant requests
            if !key_cache.contains_key(&key) {
                key_cache.insert(key, key.value(&atom));
            }

            // replace match with the related Atom value
            let val = key_cache.get(&key).expect("failed getting key value");
            dst.push_str(val);

            true
        });
        println!("{result}");
        Ok(ExitCode::SUCCESS)
    }
}
