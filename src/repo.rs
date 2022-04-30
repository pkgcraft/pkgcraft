use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::{IndexMap, IndexSet};
use once_cell::sync::Lazy;

use crate::pkg::Package;
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

    fn len(&self) -> usize {
        let mut len = 0;
        for v in self.pkgmap.values() {
            len += v.len();
        }
        len
    }

    fn is_empty(&self) -> bool {
        self.pkgmap.is_empty()
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
    pub fn from_path<P, S>(id: S, path: P) -> Result<(&'static str, Self)>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        let path = path.as_ref();
        let id = id.as_ref();

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
    pub(crate) fn from_format<P, S>(id: S, path: P, format: &str) -> Result<Self>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        let path = path.as_ref();
        let id = id.as_ref();

        match format {
            ebuild::Repo::FORMAT => Ok(Repository::Ebuild(ebuild::Repo::from_path(id, path)?)),
            fake::Repo::FORMAT => Ok(Repository::Fake(fake::Repo::from_path(id, path)?)),
            _ => Err(Error::RepoInit(format!("{id} repo: unknown format: {format}"))),
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

pub trait Repo: fmt::Debug + fmt::Display {
    fn categories(&self) -> Vec<String>;
    fn packages(&self, cat: &str) -> Vec<String>;
    fn versions(&self, cat: &str, pkg: &str) -> Vec<String>;
    fn id(&self) -> &str;
    // TODO: convert to `impl Iterator` return type once supported within traits
    // https://github.com/rust-lang/rfcs/blob/master/text/1522-conservative-impl-trait.md
    fn iter(&self) -> Box<dyn Iterator<Item = Package>>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

impl fmt::Display for Repository {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Repository::Ebuild(ref repo) => write!(f, "{}", repo),
            Repository::Fake(ref repo) => write!(f, "{}", repo),
        }
    }
}

// TODO: use a macro to create this wrapper implementation
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

    #[inline]
    fn iter(&self) -> Box<dyn Iterator<Item = Package>> {
        match self {
            Repository::Ebuild(ref repo) => repo.iter(),
            Repository::Fake(ref repo) => repo.iter(),
        }
    }

    #[inline]
    fn len(&self) -> usize {
        match self {
            Repository::Ebuild(ref repo) => repo.len(),
            Repository::Fake(ref repo) => repo.len(),
        }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        match self {
            Repository::Ebuild(ref repo) => repo.is_empty(),
            Repository::Fake(ref repo) => repo.is_empty(),
        }
    }
}
