use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::{IndexMap, IndexSet};
use once_cell::sync::Lazy;

use crate::{atom, pkg, Error, Result};

pub(crate) mod ebuild;
pub(crate) mod fake;

type VersionMap = IndexMap<String, IndexSet<String>>;
type PkgMap = IndexMap<String, VersionMap>;

#[derive(Debug, Default, PartialEq, Eq)]
struct PkgCache {
    pkgmap: PkgMap,
    atoms: IndexSet<atom::Atom>,
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
        self.atoms.len()
    }

    fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }
}

impl<'a> IntoIterator for &'a PkgCache {
    type Item = &'a atom::Atom;
    type IntoIter = PkgCacheIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PkgCacheIter {
            iter: self.atoms.iter(),
        }
    }
}

pub struct PkgCacheIter<'a> {
    iter: indexmap::set::Iter<'a, atom::Atom>,
}

impl<'a> Iterator for PkgCacheIter<'a> {
    type Item = &'a atom::Atom;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
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
    pub(crate) fn from_path<P, S>(id: S, path: P) -> Result<(&'static str, Self)>
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

    pub fn iter(&self) -> PackageIter {
        self.into_iter()
    }
}

pub enum PackageIter<'a> {
    Ebuild(ebuild::PkgIter<'a>),
    Fake(fake::PkgIter<'a>),
}

impl<'a> IntoIterator for &'a Repository {
    type Item = pkg::Package<'a>;
    type IntoIter = PackageIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Repository::Ebuild(ref repo) => PackageIter::Ebuild(repo.into_iter()),
            Repository::Fake(ref repo) => PackageIter::Fake(repo.into_iter()),
        }
    }
}

impl<'a> Iterator for PackageIter<'a> {
    type Item = pkg::Package<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            PackageIter::Ebuild(iter) => iter.next().map(pkg::Package::Ebuild),
            PackageIter::Fake(iter) => iter.next().map(pkg::Package::Fake),
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
    fn categories(&self) -> Vec<String> {
        match self {
            Repository::Ebuild(ref repo) => repo.categories(),
            Repository::Fake(ref repo) => repo.categories(),
        }
    }

    fn packages(&self, cat: &str) -> Vec<String> {
        match self {
            Repository::Ebuild(ref repo) => repo.packages(cat),
            Repository::Fake(ref repo) => repo.packages(cat),
        }
    }

    fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
        match self {
            Repository::Ebuild(ref repo) => repo.versions(cat, pkg),
            Repository::Fake(ref repo) => repo.versions(cat, pkg),
        }
    }

    fn id(&self) -> &str {
        match self {
            Repository::Ebuild(ref repo) => repo.id(),
            Repository::Fake(ref repo) => repo.id(),
        }
    }

    fn len(&self) -> usize {
        match self {
            Repository::Ebuild(ref repo) => repo.len(),
            Repository::Fake(ref repo) => repo.len(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Repository::Ebuild(ref repo) => repo.is_empty(),
            Repository::Fake(ref repo) => repo.is_empty(),
        }
    }
}

/// A repo contains a given object.
pub trait Contains<T> {
    fn contains(&self, obj: T) -> bool;
}

impl<T: AsRef<Path>> Contains<T> for Repository {
    fn contains(&self, path: T) -> bool {
        match self {
            Repository::Ebuild(ref repo) => repo.contains(path),
            Repository::Fake(ref repo) => repo.contains(path),
        }
    }
}

macro_rules! make_contains {
    ($($x:ty),*) => {$(
        impl Contains<$x> for Repository {
            fn contains(&self, obj: $x) -> bool {
                match self {
                    Repository::Ebuild(ref repo) => repo.contains(obj),
                    Repository::Fake(ref repo) => repo.contains(obj),
                }
            }
        }
    )*};
}
make_contains!(atom::Atom, &atom::Atom);
