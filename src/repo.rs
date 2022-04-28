use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::{IndexMap, IndexSet};
use once_cell::sync::Lazy;

use crate::{Error, Result};

pub(crate) mod ebuild;
mod fake;

type VersionMap = IndexMap<String, IndexSet<String>>;
type PkgMap = IndexMap<String, VersionMap>;

#[derive(Debug, Default)]
struct PkgCache {
    pkgmap: PkgMap,
}

impl PkgCache {
    fn categories(&self) -> Vec<String> {
        self.pkgmap.clone().into_keys().collect()
    }

    fn packages<S: AsRef<str>>(&self, cat: S) -> Vec<String> {
        match self.pkgmap.get(cat.as_ref()) {
            Some(pkgs) => pkgs.clone().into_keys().collect(),
            None => vec![],
        }
    }

    fn versions<S: AsRef<str>>(&self, cat: S, pkg: S) -> Vec<String> {
        match self.pkgmap.get(cat.as_ref()) {
            Some(pkgs) => match pkgs.get(pkg.as_ref()) {
                Some(vers) => vers.clone().into_iter().collect(),
                None => vec![],
            },
            None => vec![],
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Repository {
    Ebuild(ebuild::Repo),
    Fake(fake::Repo),
}

impl Repository {
    /// Determine if a given repository format is supported.
    pub(crate) fn is_supported<S: AsRef<str>>(format: S) -> Result<()> {
        let format = format.as_ref();
        match SUPPORTED_FORMATS.get(format) {
            Some(_) => Ok(()),
            None => Err(Error::RepoInit(format!("unknown repo format: {format:?}"))),
        }
    }

    /// Try to load a repository from a given path.
    pub(crate) fn from_path<P: AsRef<Path>>(id: &str, path: P) -> Result<(&'static str, Self)> {
        let path = path.as_ref();

        for format in SUPPORTED_FORMATS.iter() {
            if let Ok(repo) = Self::from_format(id, path, format) {
                return Ok((format, repo));
            }
        }

        Err(Error::InvalidRepo {
            path: PathBuf::from(path),
            error: "unknown or invalid format".to_string(),
        })
    }

    /// Try to load a certain repository type from a given path.
    pub(crate) fn from_format<P: AsRef<Path>>(id: &str, path: P, format: &str) -> Result<Self> {
        let path = path.as_ref();

        match format {
            ebuild::Repo::FORMAT => Ok(Repository::Ebuild(ebuild::Repo::from_path(id, path)?)),
            fake::Repo::FORMAT => Ok(Repository::Fake(fake::Repo::from_path(id, path)?)),
            _ => Err(Error::RepoInit(format!("{id:?} repo: unknown format: {format:?}"))),
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
    fn categories(&self) -> Vec<String>;
    fn packages(&self, cat: &str) -> Vec<String>;
    fn versions(&self, cat: &str, pkg: &str) -> Vec<String>;
    fn id(&self) -> &str;
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
    fn categories(&self) -> Vec<String> {
        match self {
            Repository::Ebuild(ref repo) => repo.categories(),
            Repository::Fake(ref repo) => repo.categories(),
        }
    }

    #[inline]
    fn packages(&self, cat: &str) -> Vec<String> {
        match self {
            Repository::Ebuild(ref repo) => repo.packages(cat),
            Repository::Fake(ref repo) => repo.packages(cat),
        }
    }

    #[inline]
    fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
        match self {
            Repository::Ebuild(ref repo) => repo.versions(cat, pkg),
            Repository::Fake(ref repo) => repo.versions(cat, pkg),
        }
    }

    #[inline]
    fn id(&self) -> &str {
        match self {
            Repository::Ebuild(ref repo) => repo.id(),
            Repository::Fake(ref repo) => repo.id(),
        }
    }
}
