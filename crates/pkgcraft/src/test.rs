#![cfg(test)]
use std::{fmt, fs};

use camino::Utf8PathBuf;
use itertools::Itertools;
use once_cell::sync::Lazy;
use serde::{de, Deserialize, Deserializer};
use serde_with::{serde_as, DisplayFromStr};

use crate::atom::{Blocker, Revision, SlotOperator, Version};
use crate::macros::build_from_paths;
use crate::set::OrderedSet;
use crate::Error;

static TOML_DATA_DIR: Lazy<Utf8PathBuf> =
    Lazy::new(|| build_from_paths!(env!("CARGO_MANIFEST_DIR"), "testdata", "toml"));

/// Explicitly initialize bash for all test executables.
#[ctor::ctor]
fn initialize() {
    Lazy::force(&crate::pkgsh::BASH);
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub(crate) struct ValidAtom {
    pub(crate) atom: String,
    pub(crate) eapis: String,
    pub(crate) category: String,
    pub(crate) package: String,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub(crate) blocker: Option<Blocker>,
    pub(crate) version: Option<Version>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub(crate) revision: Option<Revision>,
    pub(crate) slot: Option<String>,
    pub(crate) subslot: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub(crate) slot_op: Option<SlotOperator>,
    #[serde(rename = "use")]
    pub(crate) use_deps: Option<OrderedSet<String>>,
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
pub(crate) struct Intersects {
    pub(crate) vals: Vec<String>,
    pub(crate) status: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Sorted {
    pub(crate) sorted: Vec<String>,
    pub(crate) equal: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AtomData {
    pub(crate) valid: Vec<ValidAtom>,
    pub(crate) invalid: Vec<String>,
    compares: Vec<String>,
    pub(crate) intersects: Vec<Intersects>,
    pub(crate) sorting: Vec<Sorted>,
}

impl AtomData {
    pub(crate) fn load() -> crate::Result<Self> {
        let path = TOML_DATA_DIR.join("atom.toml");
        let data = fs::read_to_string(&path)
            .map_err(|e| Error::IO(format!("failed loading data: {path:?}: {e}")))?;
        toml::from_str(&data).map_err(|e| Error::IO(format!("invalid data format: {path:?}: {e}")))
    }

    pub(crate) fn compares(&self) -> ComparesIter {
        ComparesIter {
            iter: self.compares.iter(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Hashing {
    pub(crate) versions: Vec<String>,
    pub(crate) equal: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VersionData {
    compares: Vec<String>,
    pub(crate) intersects: Vec<Intersects>,
    pub(crate) sorting: Vec<Sorted>,
    pub(crate) hashing: Vec<Hashing>,
}

impl VersionData {
    pub(crate) fn load() -> crate::Result<Self> {
        let path = TOML_DATA_DIR.join("version.toml");
        let data = fs::read_to_string(&path)
            .map_err(|e| Error::IO(format!("failed loading data: {path:?}: {e}")))?;
        toml::from_str(&data).map_err(|e| Error::IO(format!("invalid data format: {path:?}: {e}")))
    }

    pub(crate) fn compares(&self) -> ComparesIter {
        ComparesIter {
            iter: self.compares.iter(),
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

/// Verify two, unordered iterables contain the same elements.
pub(crate) fn assert_unordered_eq<I, J, T, S>(a: I, b: J)
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
pub(crate) fn assert_ordered_eq<I, J, T, S>(a: I, b: J)
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
