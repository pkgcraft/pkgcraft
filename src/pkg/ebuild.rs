use std::collections::HashMap;
use std::io::{self, prelude::*};
use std::path::{Path, PathBuf};
use std::{fmt, fs};

use indexmap::IndexSet;
use once_cell::sync::Lazy;
use regex::Regex;
use scallop::source;
use scallop::variables::string_value;

use crate::atom::Atom;
use crate::eapi::Key::*;
use crate::{eapi, pkg, repo, Error, Result};

static EAPI_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^EAPI=['\"]?(?P<EAPI>[A-Za-z0-9+_.-]*)['\"]?[\t ]*(?:#.*)?").unwrap());

#[derive(Debug, Default, Clone)]
struct Metadata {
    data: HashMap<eapi::Key, String>,
}

impl Metadata {
    fn new(path: &Path, eapi: &'static eapi::Eapi) -> Result<Self> {
        // TODO: run sourcing via an external process pool returning the requested variables
        source::file(path)?;
        let mut data = HashMap::new();

        // verify sourced EAPI matches parsed EAPI
        let sourced_eapi = string_value("EAPI").unwrap_or_else(|| "0".into());
        if eapi::get_eapi(&sourced_eapi)? != eapi {
            return Err(Error::InvalidValue(format!(
                "mismatched sourced and parsed EAPIs: {sourced_eapi} != {eapi}"
            )));
        }

        // required metadata variables
        for key in eapi.mandatory_keys() {
            let val = key
                .get(eapi)
                .ok_or_else(|| Error::InvalidValue(format!("missing required value: {key}")))?;
            data.insert(*key, val);
        }

        // metadata variables that default to empty
        for key in eapi.metadata_keys().difference(eapi.mandatory_keys()) {
            key.get(eapi).and_then(|v| data.insert(*key, v));
        }

        Ok(Self { data })
    }

    fn description(&self) -> &str {
        // mandatory key guaranteed to exist
        self.data.get(&Description).unwrap()
    }

    fn slot(&self) -> &str {
        // mandatory key guaranteed to exist
        let val = self.data.get(&Slot).unwrap();
        val.split_once('/').map_or(val, |x| x.0)
    }

    fn subslot(&self) -> &str {
        // mandatory key guaranteed to exist
        let val = self.data.get(&Slot).unwrap();
        val.split_once('/').map_or(val, |x| x.1)
    }

    fn homepage(&self) -> Vec<&str> {
        let val = self.data.get(&Homepage).map(|s| s.as_str()).unwrap_or("");
        val.split_whitespace().collect()
    }

    fn keywords(&self) -> IndexSet<&str> {
        let val = self.data.get(&Keywords).map(|s| s.as_str()).unwrap_or("");
        val.split_whitespace().collect()
    }

    fn iuse(&self) -> IndexSet<&str> {
        let val = self.data.get(&Iuse).map(|s| s.as_str()).unwrap_or("");
        val.split_whitespace().collect()
    }
}

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    path: PathBuf,
    atom: Atom,
    eapi: &'static eapi::Eapi,
    repo: &'a repo::ebuild::Repo,
    data: Metadata,
}

impl PartialEq for Pkg<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for Pkg<'_> {}

impl<'a> Pkg<'a> {
    pub(crate) fn new(path: &Path, repo: &'a repo::ebuild::Repo) -> Result<Self> {
        let atom = repo.atom_from_path(path)?;
        let eapi = Pkg::get_eapi(path)?;
        let data = Metadata::new(path, eapi)?;
        Ok(Pkg {
            path: path.to_path_buf(),
            atom,
            eapi,
            repo,
            data,
        })
    }

    fn get_eapi<P: AsRef<Path>>(path: P) -> Result<&'static eapi::Eapi> {
        let mut eapi = &*eapi::EAPI0;
        let path = path.as_ref();
        let f = fs::File::open(path).map_err(|e| Error::IO(e.to_string()))?;
        let reader = io::BufReader::new(f);
        for line in reader.lines() {
            let line = line.map_err(|e| Error::IO(e.to_string()))?;
            match line.chars().next() {
                None | Some('#') => continue,
                _ => {
                    if let Some(c) = EAPI_LINE_RE.captures(&line) {
                        eapi = eapi::get_eapi(c.name("EAPI").unwrap().as_str())?;
                    }
                    break;
                }
            }
        }
        Ok(eapi)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn ebuild(&self) -> String {
        // IO errors should be caught on initialization in new().
        fs::read_to_string(&self.path).unwrap()
    }

    /// Return a package's description.
    pub fn description(&self) -> &str {
        self.data.description()
    }

    /// Return a package's slot.
    pub fn slot(&self) -> &str {
        self.data.slot()
    }

    /// Return a package's subslot.
    pub fn subslot(&self) -> &str {
        self.data.subslot()
    }

    /// Return a package's subslot.
    pub fn homepage(&self) -> Vec<&str> {
        self.data.homepage()
    }

    /// Return a package's keywords.
    pub fn keywords(&self) -> IndexSet<&str> {
        self.data.keywords()
    }

    /// Return a package's IUSE.
    pub fn iuse(&self) -> IndexSet<&str> {
        self.data.iuse()
    }
}

impl AsRef<Path> for Pkg<'_> {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

impl fmt::Display for Pkg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.path())
    }
}

impl<'a> pkg::Package for Pkg<'a> {
    type Repo = &'a repo::ebuild::Repo;

    fn atom(&self) -> &Atom {
        &self.atom
    }

    fn eapi(&self) -> &eapi::Eapi {
        self.eapi
    }

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::eapi;
    use crate::pkg::{Env, Package};
    use crate::repo::ebuild::TempRepo;

    // TODO: drop this once bash process pool support is added
    rusty_fork_test! {
        #[test]
        fn test_as_ref_path() {
            fn assert_path<P: AsRef<Path>>(pkg: P, path: &Path) {
                assert_eq!(pkg.as_ref(), path);
            }

            let t = TempRepo::new("test", None::<&str>, None).unwrap();
            let path = t.create_ebuild("cat/pkg-1", []).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_path(pkg, &path);
        }

        #[test]
        fn test_pkg_methods() {
            let t = TempRepo::new("test", None::<&str>, None).unwrap();
            let repo = &t.repo;

            // temp repo ebuild creation defaults to the latest EAPI
            let path = t.create_ebuild("cat/pkg-1", []).unwrap();
            let pkg = Pkg::new(&path, &repo).unwrap();
            assert_eq!(pkg.eapi(), &*eapi::EAPI_LATEST);
            assert_eq!(pkg.path(), &path);
            assert!(!pkg.ebuild().is_empty());

            let path = t.create_ebuild("cat/pkg-2", [("eapi", "0")]).unwrap();
            let pkg = Pkg::new(&path, &repo).unwrap();
            assert_eq!(pkg.eapi(), &*eapi::EAPI0);
            assert_eq!(pkg.path(), &path);
            assert!(!pkg.ebuild().is_empty());
        }

        #[test]
        fn test_pkg_env() {
            let t = TempRepo::new("test", None::<&str>, None).unwrap();

            // no revision
            let path = t.create_ebuild("cat/pkg-1", []).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.env(Env::P), "pkg-1");
            assert_eq!(pkg.env(Env::PN), "pkg");
            assert_eq!(pkg.env(Env::PV), "1");
            assert_eq!(pkg.env(Env::PR), "r0");
            assert_eq!(pkg.env(Env::PVR), "1");
            assert_eq!(pkg.env(Env::PF), "pkg-1");
            assert_eq!(pkg.env(Env::CATEGORY), "cat");

            // revisioned
            let path = t.create_ebuild("cat/pkg-1-r2", []).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.env(Env::P), "pkg-1");
            assert_eq!(pkg.env(Env::PN), "pkg");
            assert_eq!(pkg.env(Env::PV), "1");
            assert_eq!(pkg.env(Env::PR), "r2");
            assert_eq!(pkg.env(Env::PVR), "1-r2");
            assert_eq!(pkg.env(Env::PF), "pkg-1-r2");
            assert_eq!(pkg.env(Env::CATEGORY), "cat");

            // explicit r0 revision
            let path = t.create_ebuild("cat/pkg-2-r0", []).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.env(Env::P), "pkg-2");
            assert_eq!(pkg.env(Env::PN), "pkg");
            assert_eq!(pkg.env(Env::PV), "2");
            assert_eq!(pkg.env(Env::PR), "r0");
            assert_eq!(pkg.env(Env::PVR), "2");
            assert_eq!(pkg.env(Env::PF), "pkg-2");
            assert_eq!(pkg.env(Env::CATEGORY), "cat");
        }

        #[test]
        fn test_slot() {
            let t = TempRepo::new("test", None::<&str>, None).unwrap();

            // default (injected by create_ebuild())
            let path = t.create_ebuild("cat/pkg-1", []).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.slot(), "0");
            assert_eq!(pkg.subslot(), "0");

            // custom lacking subslot
            let path = t.create_ebuild("cat/pkg-2", [("slot", "1")]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.slot(), "1");
            assert_eq!(pkg.subslot(), "1");

            // custom with subslot
            let path = t.create_ebuild("cat/pkg-3", [("slot", "1/2")]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.slot(), "1");
            assert_eq!(pkg.subslot(), "2");
        }

        #[test]
        fn test_description() {
            let t = TempRepo::new("test", None::<&str>, None).unwrap();

            let path = t.create_ebuild("cat/pkg-1", [("description", "desc")]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.description(), "desc");
        }

        #[test]
        fn test_homepage() {
            let t = TempRepo::new("test", None::<&str>, None).unwrap();

            // none
            let path = t.create_ebuild("cat/pkg-1", [("homepage", "-")]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert!(pkg.homepage().is_empty());

            // single line
            let path = t.create_ebuild("cat/pkg-1", [("homepage", "home")]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.homepage(), ["home"]);

            // multiple lines
            let val = indoc::indoc! {"
                a
                b
                c
            "};
            let path = t.create_ebuild("cat/pkg-1", [("homepage", val)]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.homepage(), ["a", "b", "c"]);
        }

        #[test]
        fn test_keywords() {
            let t = TempRepo::new("test", None::<&str>, None).unwrap();

            // none
            let path = t.create_ebuild("cat/pkg-1", []).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert!(pkg.keywords().is_empty());

            // single line
            let path = t.create_ebuild("cat/pkg-1", [("keywords", "a b")]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.keywords().iter().cloned().collect::<Vec<&str>>(), ["a", "b"]);

            // multiple lines
            let val = indoc::indoc! {"
                a
                b
                c
            "};
            let path = t.create_ebuild("cat/pkg-1", [("keywords", val)]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.keywords().iter().cloned().collect::<Vec<&str>>(), ["a", "b", "c"]);
        }

        #[test]
        fn test_iuse() {
            let t = TempRepo::new("test", None::<&str>, None).unwrap();

            // none
            let path = t.create_ebuild("cat/pkg-1", []).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert!(pkg.iuse().is_empty());

            // single line
            let path = t.create_ebuild("cat/pkg-1", [("iuse", "a b")]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.iuse().iter().cloned().collect::<Vec<&str>>(), ["a", "b"]);

            // multiple lines
            let val = indoc::indoc! {"
                a
                b
                c
            "};
            let path = t.create_ebuild("cat/pkg-1", [("iuse", val)]).unwrap();
            let pkg = Pkg::new(&path, &t.repo).unwrap();
            assert_eq!(pkg.iuse().iter().cloned().collect::<Vec<&str>>(), ["a", "b", "c"]);
        }
    }
}
