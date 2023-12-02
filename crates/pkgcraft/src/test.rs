use std::{env, fmt, fs};

use assert_cmd::Command;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use walkdir::WalkDir;

use crate::config::Config;
use crate::dep::{Blocker, Dep, Revision, SlotOperator, UseDep, Version};
use crate::macros::build_from_paths;
use crate::repo::PkgRepository;
use crate::types::SortedSet;
use crate::Error;

/// Flag denoting a test suite, either internally or externally, is running.
pub(crate) static TESTING: Lazy<bool> = Lazy::new(|| {
    cfg!(test)
        || std::env::var("CARGO_MANIFEST_DIR").is_ok()
        || std::env::var("PKGCRAFT_TEST").is_ok()
});

/// Construct a Command from a given string.
pub fn cmd<S: AsRef<str>>(cmd: S) -> Command {
    let args: Vec<_> = cmd.as_ref().split_whitespace().collect();
    let mut cmd = Command::cargo_bin(args[0]).unwrap();
    cmd.args(&args[1..]);
    // disable config loading by default for pkgcraft-related commands
    cmd.env("PKGCRAFT_NO_CONFIG", "1");
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
    Lazy::force(&crate::shell::BASH);
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
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub version: Option<Version>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub revision: Option<Revision>,
    pub slot: Option<String>,
    pub subslot: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub slot_op: Option<SlotOperator>,
    #[serde(rename = "use")]
    pub use_deps: Option<SortedSet<UseDep<String>>>,
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
    pub fn load(path: &Utf8Path) -> crate::Result<Self> {
        let data = fs::read_to_string(path)
            .map_err(|e| Error::IO(format!("failed loading data: {path}: {e}")))?;
        toml::from_str(&data).map_err(|e| Error::IO(format!("invalid data format: {path}: {e}")))
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
    pub valid: Vec<String>,
    pub invalid: Vec<String>,
    compares: Vec<String>,
    pub intersects: Vec<Intersects>,
    pub sorting: Vec<Sorted>,
    pub hashing: Vec<Hashing>,
}

impl VersionToml {
    pub fn load(path: &Utf8Path) -> crate::Result<Self> {
        let data = fs::read_to_string(path)
            .map_err(|e| Error::IO(format!("failed loading data: {path}: {e}")))?;
        toml::from_str(&data).map_err(|e| Error::IO(format!("invalid data format: {path}: {e}")))
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

#[derive(Debug)]
pub struct TestData {
    pub path: Utf8PathBuf,
    pub config: Config,
    pub dep_toml: DepToml,
    pub version_toml: VersionToml,
}

impl TestData {
    pub fn ebuild_repo(&self, name: &str) -> Option<&crate::repo::ebuild::Repo> {
        self.config
            .repos
            .get(name)
            .and_then(|r| r.as_ebuild().map(|r| r.as_ref()))
    }

    pub fn ebuild_raw_pkg<'a>(&'a self, s: &str) -> Option<crate::pkg::ebuild::raw::Pkg<'a>> {
        let dep: Dep = s.parse().expect("invalid dep");
        self.ebuild_repo(dep.repo().expect("dep missing repo id"))
            .and_then(|r| r.iter_raw_restrict(&dep).next())
    }

    pub fn ebuild_pkg<'a>(&'a self, s: &str) -> Option<crate::pkg::ebuild::Pkg<'a>> {
        let dep: Dep = s.parse().expect("invalid dep");
        self.ebuild_repo(dep.repo().expect("dep missing repo id"))
            .and_then(|r| r.iter_restrict(&dep).next())
    }
}

pub static TEST_DATA: Lazy<TestData> = Lazy::new(|| {
    let path = build_from_paths!(env!("CARGO_MANIFEST_DIR"), "testdata");

    // load valid repos from test data, ignoring purposefully broken ones
    let mut config = Config::new("pkgcraft", "");
    for entry in WalkDir::new(path.join("repos")).sort_by_file_name() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_str().unwrap();
        let path = entry.path().to_str().unwrap();
        config.add_repo_path(name, 0, path, false).ok();
    }

    TestData {
        path: path.clone(),
        config,
        dep_toml: DepToml::load(&path.join("toml/dep.toml")).unwrap(),
        version_toml: VersionToml::load(&path.join("toml/version.toml")).unwrap(),
    }
});

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
