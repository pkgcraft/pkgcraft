use std::collections::{HashMap, HashSet};
use std::fmt;
use std::iter;
use std::path::{Path, PathBuf};

use indexmap::IndexSet;
use once_cell::sync::Lazy;

use crate::error::Error;

pub(crate) mod ebuild;
mod fake;

type VersionMap = HashMap<String, HashSet<String>>;
type PkgMap = HashMap<String, VersionMap>;
type StringIter<'a> = Box<dyn Iterator<Item = &'a String> + 'a>;

#[derive(Debug, Default)]
struct PkgCache {
    pkgmap: PkgMap,
}

impl PkgCache {
    fn categories(&self) -> StringIter {
        Box::new(self.pkgmap.keys())
    }

    fn packages<S: AsRef<str>>(&self, cat: S) -> StringIter {
        match self.pkgmap.get(cat.as_ref()) {
            Some(pkgs) => Box::new(pkgs.keys()),
            None => Box::new(iter::empty::<&String>()),
        }
    }

    fn versions<S: AsRef<str>>(&self, cat: S, pkg: S) -> StringIter {
        match self.pkgmap.get(cat.as_ref()) {
            Some(pkgs) => match pkgs.get(pkg.as_ref()) {
                Some(vers) => Box::new(vers.iter()),
                None => Box::new(iter::empty::<&String>()),
            },
            None => Box::new(iter::empty::<&String>()),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Repository {
    Ebuild(ebuild::Repo),
    Fake(fake::Repo),
}

impl Repository {
    pub(crate) fn is_supported<S: AsRef<str>>(format: S) -> crate::Result<()> {
        let format = format.as_ref();
        match SUPPORTED_FORMATS.get(format) {
            Some(_) => Ok(()),
            None => Err(Error::RepoInit(format!(
                "unknown repo format: {:?}",
                format
            ))),
        }
    }

    pub(crate) fn from_path<P: AsRef<Path>>(
        id: &str,
        path: P,
    ) -> crate::Result<(&'static str, Self)> {
        let path = path.as_ref();

        for format in SUPPORTED_FORMATS.iter() {
            if let Ok(repo) = Self::from_format(id, path, format) {
                return Ok((format, repo));
            }
        }

        Err(Error::RepoInvalid {
            path: PathBuf::from(path),
            error: "unknown or invalid format".to_string(),
        })
    }

    pub(crate) fn from_format<P: AsRef<Path>>(
        id: &str,
        path: P,
        format: &str,
    ) -> crate::Result<Self> {
        let path = path.as_ref();

        match format {
            ebuild::Repo::FORMAT => Ok(Repository::Ebuild(ebuild::Repo::from_path(id, path)?)),
            fake::Repo::FORMAT => Ok(Repository::Fake(fake::Repo::from_path(id, path)?)),
            _ => Err(Error::RepoInit(format!(
                "{:?} repo: unknown format: {:?}",
                id, format
            ))),
        }
    }
}

// externally supported repo formats
#[rustfmt::skip]
static SUPPORTED_FORMATS: Lazy<IndexSet<&'static str>> = Lazy::new(|| {
    [
        ebuild::Repo::FORMAT,
        fake::Repo::FORMAT,
    ].iter().cloned().collect()
});

pub(crate) trait Repo: fmt::Debug + fmt::Display {
    // TODO: convert to `impl Iterator` return type once supported within traits
    // https://github.com/rust-lang/rfcs/blob/master/text/1522-conservative-impl-trait.md
    fn categories(&mut self) -> StringIter;
    fn packages(&mut self, cat: &str) -> StringIter;
    fn versions(&mut self, cat: &str, pkg: &str) -> StringIter;
}

impl fmt::Display for Repository {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Repository::Ebuild(ref repo) => write!(f, "{}", repo),
            Repository::Fake(ref repo) => write!(f, "{}", repo),
        }
    }
}

impl Repo for Repository {
    #[inline]
    fn categories(&mut self) -> StringIter {
        match self {
            Repository::Ebuild(ref mut repo) => repo.categories(),
            Repository::Fake(ref mut repo) => repo.categories(),
        }
    }

    #[inline]
    fn packages(&mut self, cat: &str) -> StringIter {
        match self {
            Repository::Ebuild(ref mut repo) => repo.packages(cat),
            Repository::Fake(ref mut repo) => repo.packages(cat),
        }
    }

    #[inline]
    fn versions(&mut self, cat: &str, pkg: &str) -> StringIter {
        match self {
            Repository::Ebuild(ref mut repo) => repo.versions(cat, pkg),
            Repository::Fake(ref mut repo) => repo.versions(cat, pkg),
        }
    }
}
