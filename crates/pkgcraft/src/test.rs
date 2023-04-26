#![cfg(any(test, feature = "test"))]
use std::{env, fmt, fs};

use assert_cmd::Command;
use camino::Utf8PathBuf;
use itertools::Itertools;
use once_cell::sync::Lazy;
use serde::{de, Deserialize, Deserializer};
use serde_with::{serde_as, DisplayFromStr};

use crate::dep::{Blocker, Revision, SlotOperator, Version};
use crate::macros::build_from_paths;
use crate::set::OrderedSet;
use crate::Error;

pub static TEST_DATA_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| build_from_paths!(env!("CARGO_MANIFEST_DIR"), "testdata"));

/// Construct a Command from a given string.
pub fn cmd(cmd: &str) -> Command {
    let args: Vec<_> = cmd.split_whitespace().collect();
    let mut cmd = Command::cargo_bin(args[0]).unwrap();
    cmd.args(&args[1..]);
    cmd
}

/// Initialization for all test executables.
#[cfg(test)]
#[ctor::ctor]
fn initialize() {
    // verify running under `cargo nextest` ignoring benchmark runs
    if !env::args().any(|x| x == "--bench") {
        env::var("NEXTEST").expect("tests must be run via cargo-nextest");
    }
    // initialize bash
    Lazy::force(&crate::pkgsh::BASH);
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct ValidDep {
    pub dep: String,
    pub eapis: String,
    pub category: String,
    pub package: String,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub blocker: Option<Blocker>,
    pub version: Option<Version>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub revision: Option<Revision>,
    pub slot: Option<String>,
    pub subslot: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub slot_op: Option<SlotOperator>,
    #[serde(rename = "use")]
    pub use_deps: Option<OrderedSet<String>>,
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        Version::new_with_op(&s).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for OrderedSet<String> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vals: Vec<String> = Deserialize::deserialize(deserializer)?;
        Ok(vals.into_iter().collect())
    }
}

#[derive(Debug, Deserialize)]
pub struct Intersects {
    pub vals: Vec<String>,
    pub status: bool,
}

#[derive(Debug, Deserialize)]
pub struct Sorted {
    pub sorted: Vec<String>,
    pub equal: bool,
}

#[derive(Debug, Deserialize)]
pub struct DepToml {
    pub valid: Vec<ValidDep>,
    pub invalid: Vec<String>,
    compares: Vec<String>,
    pub intersects: Vec<Intersects>,
    pub sorting: Vec<Sorted>,
}

impl DepToml {
    pub fn load() -> crate::Result<Self> {
        let path = TEST_DATA_PATH.join("toml/dep.toml");
        let data = fs::read_to_string(&path)
            .map_err(|e| Error::IO(format!("failed loading data: {path:?}: {e}")))?;
        toml::from_str(&data).map_err(|e| Error::IO(format!("invalid data format: {path:?}: {e}")))
    }

    pub fn compares(&self) -> ComparesIter {
        ComparesIter { iter: self.compares.iter() }
    }
}

#[derive(Debug, Deserialize)]
pub struct Hashing {
    pub versions: Vec<String>,
    pub equal: bool,
}

#[derive(Debug, Deserialize)]
pub struct VersionToml {
    compares: Vec<String>,
    pub intersects: Vec<Intersects>,
    pub sorting: Vec<Sorted>,
    pub hashing: Vec<Hashing>,
}

impl VersionToml {
    pub fn load() -> crate::Result<Self> {
        let path = TEST_DATA_PATH.join("toml/version.toml");
        let data = fs::read_to_string(&path)
            .map_err(|e| Error::IO(format!("failed loading data: {path:?}: {e}")))?;
        toml::from_str(&data).map_err(|e| Error::IO(format!("invalid data format: {path:?}: {e}")))
    }

    pub fn compares(&self) -> ComparesIter {
        ComparesIter { iter: self.compares.iter() }
    }
}

pub struct ComparesIter<'a> {
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

/// Verify two, unordered iterables contain the same elements.
pub fn assert_unordered_eq<I, J, T, S>(a: I, b: J)
where
    I: IntoIterator<Item = T>,
    J: IntoIterator<Item = S>,
    T: PartialEq<S> + Ord + fmt::Debug,
    S: PartialEq<T> + Ord + fmt::Debug,
{
    let mut a: Vec<_> = a.into_iter().collect();
    let mut b: Vec<_> = b.into_iter().collect();
    a.sort();
    b.sort();
    assert_eq!(a, b, "{a:?} != {b:?}");
}

/// Verify two, ordered iterables are equal.
pub fn assert_ordered_eq<I, J, T, S>(a: I, b: J)
where
    I: IntoIterator<Item = T>,
    J: IntoIterator<Item = S>,
    T: PartialEq<S> + Ord + fmt::Debug,
    S: PartialEq<T> + Ord + fmt::Debug,
{
    let a: Vec<_> = a.into_iter().collect();
    let b: Vec<_> = b.into_iter().collect();
    assert_eq!(a, b, "{a:?} != {b:?}");
}
