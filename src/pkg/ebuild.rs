use std::io::{self, prelude::*};
use std::path::{Path, PathBuf};
use std::{fmt, fs};

use once_cell::sync::Lazy;
use regex::Regex;

use crate::atom::Atom;
use crate::{eapi, pkg, repo, Error, Result};

static EAPI_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^EAPI=['\"]?(?P<EAPI>[A-Za-z0-9+_.-]*)['\"]?[\t ]*(?:#.*)?").unwrap());

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    path: PathBuf,
    atom: Atom,
    eapi: &'static eapi::Eapi,
    repo: &'a repo::ebuild::Repo,
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
        let eapi = Pkg::get_eapi(&path)?;
        Ok(Pkg {
            path: path.to_path_buf(),
            atom,
            eapi,
            repo,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn ebuild(&self) -> String {
        // IO errors should be caught on initialization in new().
        fs::read_to_string(&self.path).unwrap()
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
    use std::collections::HashMap;
    use std::path::Path;

    use super::*;
    use crate::eapi;
    use crate::pkg::Package;
    use crate::repo::ebuild::TempRepo;

    #[test]
    fn test_as_ref_path() {
        fn assert_path<P: AsRef<Path>>(pkg: P, path: &Path) {
            assert_eq!(pkg.as_ref(), path);
        }

        let t = TempRepo::new("test", None::<&str>, None).unwrap();
        let path = t.create_ebuild("cat/pkg-1", None).unwrap();
        let pkg = Pkg::new(&path, &t.repo).unwrap();
        assert_path(pkg, &path);
    }

    #[test]
    fn test_pkg_methods() {
        let t = TempRepo::new("test", None::<&str>, None).unwrap();
        let repo = &t.repo;

        // temp repo ebuild creation defaults to the latest EAPI
        let path = t.create_ebuild("cat/pkg-1", None).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.eapi(), &*eapi::EAPI_LATEST);
        assert_eq!(pkg.path(), &path);
        assert!(!pkg.ebuild().is_empty());

        let data = HashMap::from([("eapi", "0")]);
        let path = t.create_ebuild("cat/pkg-2", Some(data)).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.eapi(), &*eapi::EAPI0);
        assert_eq!(pkg.path(), &path);
        assert!(!pkg.ebuild().is_empty());
    }
}
