use std::{fs, io, process};

use camino::{Utf8Path, Utf8PathBuf};
use camino_tempfile::Utf8TempDir;
use itertools::Itertools;
use serde::Deserialize;
use serde_with::{DisplayFromStr, serde_as};
use walkdir::{DirEntry, WalkDir};

use crate::Error;
use crate::config::Config;
use crate::dep::{Blocker, Revision, SlotOperator, UseDep, Version};
use crate::macros::build_path;
use crate::repo::{EbuildRepo, Repo, Repository};
use crate::types::SortedSet;
use crate::utils::relpath_utf8;

/// Define a function to support testing a given list of commands.
#[macro_export]
macro_rules! define_cmd {
    ($($cmd:expr),+) => {
        /// Construct a Command from a given string.
        pub(crate) fn cmd<S: AsRef<str>>(cmd: S) -> assert_cmd::Command {
            let mut args = cmd.as_ref().split_whitespace();
            let mut cmd = match args.next() {
                $(Some($cmd) => assert_cmd::cargo::cargo_bin_cmd!($cmd),)+
                Some(x) => unreachable!("unknown command: {x}"),
                None => unreachable!("invalid command"),
            };
            cmd.args(args);
            // disable config loading by default
            cmd.env("PKGCRAFT_CONFIG", "");
            cmd.env("PORTAGE_CONFIG", "");
            cmd
        }
    };
}
pub use define_cmd;

/// Initialization for test executables.
#[ctor::ctor]
fn initialize() {
    // initialize shell for internal tests
    #[cfg(test)]
    crate::shell::init().unwrap();

    // ignore custom TMPDIR exports that would alter std::env::temp_dir()
    tempfile::env::override_temp_dir(&std::env::temp_dir());
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
    pub use_deps: Option<SortedSet<UseDep>>,
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
        toml::from_str(&data)
            .map_err(|e| Error::IO(format!("invalid data format: {path}: {e}")))
    }

    pub fn compares(&self) -> ComparesIter<'_> {
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
        toml::from_str(&data)
            .map_err(|e| Error::IO(format!("invalid data format: {path}: {e}")))
    }

    pub fn compares(&self) -> ComparesIter<'_> {
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
    path: Utf8PathBuf,
    config: Config,
    pub dep_toml: DepToml,
    pub version_toml: VersionToml,
}

impl TestData {
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    pub fn repo(&self, name: &str) -> crate::Result<&Repo> {
        self.config.repos().get(name)
    }

    pub fn ebuild_repo(&self, name: &str) -> crate::Result<&EbuildRepo> {
        self.repo(name).and_then(|repo| {
            repo.as_ebuild()
                .ok_or_else(|| Error::InvalidValue(format!("not an ebuild repo: {repo}")))
        })
    }
}

pub fn test_data_path() -> Utf8PathBuf {
    build_path!(env!("CARGO_MANIFEST_DIR"), "testdata")
}

pub fn test_data() -> TestData {
    let path = test_data_path();

    // load valid repos from test data, ignoring purposefully broken ones
    let mut config = Config::new("pkgcraft", "");
    for entry in WalkDir::new(path.join("repos/valid"))
        .min_depth(1)
        .max_depth(1)
        .sort_by_file_name()
    {
        let entry = entry.unwrap();
        let name = entry.file_name().to_str().unwrap();
        let path = entry.path().to_str().unwrap();
        let repo = Repo::from_path(name, path, 0).unwrap();
        config.add_repo(repo).unwrap();
    }

    TestData {
        path: path.clone(),
        config,
        dep_toml: DepToml::load(&path.join("toml/dep.toml")).unwrap(),
        version_toml: VersionToml::load(&path.join("toml/version.toml")).unwrap(),
    }
}

#[derive(Debug)]
pub struct TestDataPatched {
    _tmpdir: Utf8TempDir,
    config: Config,
}

impl TestDataPatched {
    pub fn repo(&self, name: &str) -> crate::Result<&Repo> {
        self.config.repos().get(name)
    }

    pub fn ebuild_repo(&self, name: &str) -> crate::Result<&EbuildRepo> {
        self.repo(name).and_then(|repo| {
            repo.as_ebuild()
                .ok_or_else(|| Error::InvalidValue(format!("not an ebuild repo: {repo}")))
        })
    }
}

/// Determine if a file is a patch or script.
fn is_change(entry: &DirEntry) -> bool {
    let path = entry.path();
    path.is_file()
        && path
            .file_name()
            .map(|s| s == "fix.patch" || s == "fix.sh")
            .unwrap_or_default()
}

pub fn test_data_patched() -> TestDataPatched {
    let tmpdir = Utf8TempDir::new().unwrap();
    let mut config = Config::new("pkgcraft", "");

    // generate temporary repos with changes applied
    let data = test_data();
    for (name, repo) in data.config.repos() {
        let changes: Vec<Utf8PathBuf> = WalkDir::new(repo.path())
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
            .filter(is_change)
            .map(|entry| entry.path().to_path_buf())
            .map(|path| path.try_into())
            .try_collect()
            .unwrap();

        if !changes.is_empty() {
            let old_path = repo.path();
            let new_path = tmpdir.path().join(name);

            for entry in WalkDir::new(old_path) {
                let entry = entry.unwrap();
                let src = Utf8Path::from_path(entry.path()).unwrap();
                let dest = new_path.join(src.strip_prefix(old_path).unwrap());

                // create directories and copy files skipping change files
                if src.is_dir() {
                    fs::create_dir(&dest)
                        .unwrap_or_else(|e| panic!("failed creating dir {dest}: {e}"));
                } else if src.is_file() && !is_change(&entry) {
                    // ignore missing transient metadata cache files
                    if let Err(e) = fs::copy(src, &dest)
                        && e.kind() != io::ErrorKind::NotFound
                    {
                        unreachable!("failed copying {src} to {dest}: {e}");
                    }
                }
            }

            // apply changes to new repo
            for change in &changes {
                let relpath = relpath_utf8(change.parent().unwrap(), old_path).unwrap();
                let dir = new_path.join(relpath);
                let name = change.file_name().unwrap();

                let status = if name == "fix.patch" {
                    process::Command::new("patch")
                        .arg("-p1")
                        .arg("-F0")
                        .arg("-N")
                        .arg("--backup-if-mismatch")
                        .stdin(fs::File::open(change).unwrap())
                        .current_dir(&dir)
                        .status()
                        .unwrap()
                } else {
                    process::Command::new(change)
                        .current_dir(&dir)
                        .status()
                        .unwrap()
                };
                assert!(status.success(), "failed applying: {change}");

                // TODO: Switch to using a patch option rejecting mismatches if upstream ever
                // supports that.
                //
                // verify no backup files were created due to mismatched patches
                let mut files = fs::read_dir(&dir).unwrap().filter_map(Result::ok);
                if files.any(|e| {
                    e.path()
                        .extension()
                        .map(|s| s == "orig")
                        .unwrap_or_default()
                }) {
                    unreachable!(
                        "mismatched patch: {}",
                        change.strip_prefix(&tmpdir).unwrap()
                    );
                }
            }

            let repo = Repo::from_path(name, new_path, 0).unwrap();
            config.add_repo(repo).unwrap();
        }
    }

    TestDataPatched { _tmpdir: tmpdir, config }
}

/// Verify two, ordered iterables are equal.
#[macro_export]
macro_rules! assert_ordered_eq {
    ($iter1:expr, $iter2:expr, $msg:expr) => {{
        let a: Vec<_> = $iter1.into_iter().collect();
        let b: Vec<_> = $iter2.into_iter().collect();
        let msg = $msg;
        pretty_assertions::assert_eq!(a, b, "{msg}");
    }};

    ($iter1:expr, $iter2:expr $(,)?) => {{
        assert_ordered_eq!($iter1, $iter2, "");
    }};
}
pub use assert_ordered_eq;

/// Verify two, unordered iterables contain the same elements.
#[macro_export]
macro_rules! assert_unordered_eq {
    ($iter1:expr, $iter2:expr, $msg:expr) => {{
        let mut a: Vec<_> = $iter1.into_iter().collect();
        let mut b: Vec<_> = $iter2.into_iter().collect();
        a.sort();
        b.sort();
        let msg = $msg;
        pretty_assertions::assert_eq!(a, b, "{msg}");
    }};

    ($iter1:expr, $iter2:expr $(,)?) => {{
        assert_unordered_eq!($iter1, $iter2, "");
    }};
}
pub use assert_unordered_eq;

/// Assert an error matches a given regular expression for testing.
#[macro_export]
macro_rules! assert_err_re {
    ($result:expr, $pattern:expr) => {
        $crate::test::assert_err_re!($result, $pattern, "");
    };
    ($result:expr, $pattern:expr, $msg:expr) => {
        let err = $result.unwrap_err().to_string();
        let re = ::regex::Regex::new(&$pattern).unwrap();
        let err_msg = format!("{err:?} does not match regex: {re:?}");
        if $msg.is_empty() {
            assert!(re.is_match(&err), "{}", err_msg);
        } else {
            assert!(re.is_match(&err), "{}", format!("{err_msg}: {}", $msg));
        }
    };
}
pub use assert_err_re;

/// Assert tracing logs match a regular expression.
#[macro_export]
macro_rules! assert_logs_re {
    ($pattern:expr) => {
        let s = format!($pattern);
        let re = ::regex::Regex::new(&s).unwrap();
        logs_assert(|lines: &[&str]| {
            if lines.iter().any(|l| re.is_match(l)) {
                Ok(())
            } else {
                Err(format!("unmatched log regex: {s}"))
            }
        });
    };
}
pub use assert_logs_re;
