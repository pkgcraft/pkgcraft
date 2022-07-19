#![cfg(test)]
use std::fs;
use std::str::FromStr;

use camino::Utf8PathBuf;
use ctor::ctor;
use itertools::Itertools;
use once_cell::sync::Lazy;
use serde::{de, Deserialize, Deserializer};

use crate::macros::build_from_paths;
use crate::pkgsh::bash_init;
use crate::{atom, Error};

/// Initialize bash for all test executables.
#[ctor]
fn initialize() {
    bash_init();
}

static TOML_DATA_DIR: Lazy<Utf8PathBuf> =
    Lazy::new(|| build_from_paths!(env!("CARGO_MANIFEST_DIR"), "testdata", "toml"));

#[derive(Debug, Deserialize)]
pub(crate) struct Atom {
    pub(crate) atom: String,
    pub(crate) eapis: String,
    pub(crate) category: String,
    pub(crate) package: String,
    pub(crate) version: Option<atom::Version>,
    pub(crate) slot: Option<String>,
    pub(crate) subslot: Option<String>,
    pub(crate) slot_op: Option<atom::SlotOperator>,
}

impl<'de> Deserialize<'de> for atom::Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        atom::parse::version_with_op(s).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for atom::SlotOperator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        atom::SlotOperator::from_str(s).map_err(de::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Atoms {
    pub(crate) valid: Vec<Atom>,
    pub(crate) invalid: Vec<(String, String)>,
    sorting: Vec<(Vec<String>, Vec<String>)>,
}

impl Atoms {
    pub(crate) fn load() -> crate::Result<Self> {
        let path = TOML_DATA_DIR.join("atoms.toml");
        let data = fs::read_to_string(&path)
            .map_err(|e| Error::IO(format!("failed loading data: {path:?}: {e}")))?;
        toml::from_str(&data).map_err(|e| Error::IO(format!("invalid data format: {path:?}: {e}")))
    }

    pub(crate) fn sorting(&self) -> SortIter {
        SortIter {
            iter: self.sorting.iter(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Versions {
    compares: Vec<String>,
    sorting: Vec<(Vec<String>, Vec<String>)>,
}

impl Versions {
    pub(crate) fn load() -> crate::Result<Self> {
        let path = TOML_DATA_DIR.join("versions.toml");
        let data = fs::read_to_string(&path)
            .map_err(|e| Error::IO(format!("failed loading data: {path:?}: {e}")))?;
        toml::from_str(&data).map_err(|e| Error::IO(format!("invalid data format: {path:?}: {e}")))
    }

    pub(crate) fn compares(&self) -> ComparesIter {
        ComparesIter {
            iter: self.compares.iter(),
        }
    }

    pub(crate) fn sorting(&self) -> SortIter {
        SortIter {
            iter: self.sorting.iter(),
        }
    }
}

pub(crate) struct ComparesIter<'a> {
    iter: std::slice::Iter<'a, String>,
}

impl<'a> Iterator for ComparesIter<'a> {
    // format: (string expression, (lhs, op, rhs))
    type Item = (&'a str, (&'a str, &'a str, &'a str));

    fn next(&mut self) -> Option<Self::Item> {
        // forcibly panic for wrong data format
        self.iter
            .next()
            .map(|s| (s.as_str(), s.split(' ').collect_tuple().unwrap()))
    }
}

pub(crate) struct SortIter<'a> {
    // format: (unsorted, sorted)
    iter: std::slice::Iter<'a, (Vec<String>, Vec<String>)>,
}

impl<'a> Iterator for SortIter<'a> {
    type Item = (Vec<&'a str>, Vec<&'a str>);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(unsorted, expected)| {
            let unsorted: Vec<_> = unsorted.iter().map(|s| s.as_str()).collect();
            let expected: Vec<_> = expected.iter().map(|s| s.as_str()).collect();
            (unsorted, expected)
        })
    }
}

/// Compare two iterables via sorted lists.
pub(crate) fn eq_sorted<I, J, T, S>(a: I, b: J) -> bool
where
    I: IntoIterator<Item = T>,
    J: IntoIterator<Item = S>,
    T: PartialEq<S> + Ord,
    S: PartialEq<T> + Ord,
{
    let mut a: Vec<_> = a.into_iter().collect();
    let mut b: Vec<_> = b.into_iter().collect();
    a.sort();
    b.sort();

    a == b
}
