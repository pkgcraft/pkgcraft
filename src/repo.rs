use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::{IndexMap, IndexSet};
use once_cell::sync::Lazy;
use tracing::warn;

use crate::pkg::Pkg;
use crate::{atom, Error, Result};

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

impl<'a> FromIterator<&'a str> for PkgCache {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> Self {
        let mut pkgmap = PkgMap::new();
        let mut atoms = IndexSet::<atom::Atom>::new();
        for s in iter {
            match atom::parse::cpv(s) {
                Ok(a) => {
                    atoms.insert(a);
                }
                Err(e) => warn!("{e}"),
            }
        }

        atoms.sort();

        for a in &atoms {
            pkgmap
                .entry(a.category().into())
                .or_insert_with(VersionMap::new)
                .entry(a.package().into())
                .or_insert_with(IndexSet::new)
                .insert(a.version().unwrap().into());
        }

        PkgCache { pkgmap, atoms }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Repo {
    Ebuild(ebuild::Repo),
    Fake(fake::Repo),
}

make_repo_traits!(Repo);

impl Repo {
    /// Determine if a given repo format is supported.
    pub(crate) fn is_supported<S: AsRef<str>>(format: S) -> Result<()> {
        let format = format.as_ref();
        match SUPPORTED_FORMATS.get(format) {
            Some(_) => Ok(()),
            None => Err(Error::RepoInit(format!("unknown repo format: {format:?}"))),
        }
    }

    /// Try to load a repo from a given path.
    pub(crate) fn from_path<P, S>(id: S, priority: i32, path: P) -> Result<(&'static str, Self)>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        let path = path.as_ref();
        let id = id.as_ref();

        for format in SUPPORTED_FORMATS.iter() {
            if let Ok(repo) = Self::from_format(id, priority, path, format) {
                return Ok((format, repo));
            }
        }

        Err(Error::InvalidRepo {
            path: PathBuf::from(path),
            error: "unknown or invalid format".to_string(),
        })
    }

    /// Try to load a certain repo type from a given path.
    pub(crate) fn from_format<P, S>(id: S, priority: i32, path: P, format: &str) -> Result<Self>
    where
        P: AsRef<Path>,
        S: AsRef<str>,
    {
        let path = path.as_ref();
        let id = id.as_ref();

        match format {
            ebuild::Repo::FORMAT => Ok(Self::Ebuild(ebuild::Repo::from_path(id, priority, path)?)),
            fake::Repo::FORMAT => Ok(Self::Fake(fake::Repo::from_path(id, priority, path)?)),
            _ => Err(Error::RepoInit(format!("{id} repo: unknown format: {format}"))),
        }
    }

    pub fn iter(&self) -> PackageIter {
        self.into_iter()
    }
}

#[allow(clippy::large_enum_variant)]
pub enum PackageIter<'a> {
    Ebuild(ebuild::PkgIter<'a>),
    Fake(fake::PkgIter<'a>),
}

impl<'a> IntoIterator for &'a Repo {
    type Item = Pkg<'a>;
    type IntoIter = PackageIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Repo::Ebuild(ref repo) => PackageIter::Ebuild(repo.into_iter()),
            Repo::Fake(ref repo) => PackageIter::Fake(repo.into_iter()),
        }
    }
}

impl<'a> Iterator for PackageIter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ebuild(iter) => iter.next().map(Pkg::Ebuild),
            Self::Fake(iter) => iter.next().map(Pkg::Fake),
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

pub trait Repository: fmt::Debug + fmt::Display + PartialEq + Eq + PartialOrd + Ord {
    // TODO: add Iterator type and iter() when GATs are stabilized
    // https://github.com/rust-lang/rust/issues/44265
    fn categories(&self) -> Vec<String>;
    fn packages(&self, cat: &str) -> Vec<String>;
    fn versions(&self, cat: &str, pkg: &str) -> Vec<String>;
    fn id(&self) -> &str;
    fn priority(&self) -> i32;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

#[derive(Debug)]
pub enum BorrowedRepo<'a> {
    Ebuild(&'a ebuild::Repo),
    Fake(&'a fake::Repo),
}

make_repo_traits!(BorrowedRepo<'_>);

macro_rules! make_repo {
    ($($x:ty),*) => {
        $(
            impl fmt::Display for $x {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    match self {
                        Self::Ebuild(ref repo) => write!(f, "{}", repo),
                        Self::Fake(ref repo) => write!(f, "{}", repo),
                    }
                }
            }

            impl Repository for $x {
                fn categories(&self) -> Vec<String> {
                    match self {
                        Self::Ebuild(ref repo) => repo.categories(),
                        Self::Fake(ref repo) => repo.categories(),
                    }
                }

                fn packages(&self, cat: &str) -> Vec<String> {
                    match self {
                        Self::Ebuild(ref repo) => repo.packages(cat),
                        Self::Fake(ref repo) => repo.packages(cat),
                    }
                }

                fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
                    match self {
                        Self::Ebuild(ref repo) => repo.versions(cat, pkg),
                        Self::Fake(ref repo) => repo.versions(cat, pkg),
                    }
                }

                fn id(&self) -> &str {
                    match self {
                        Self::Ebuild(ref repo) => repo.id(),
                        Self::Fake(ref repo) => repo.id(),
                    }
                }

                fn priority(&self) -> i32 {
                    match self {
                        Self::Ebuild(ref repo) => repo.priority(),
                        Self::Fake(ref repo) => repo.priority(),
                    }
                }

                fn len(&self) -> usize {
                    match self {
                        Self::Ebuild(ref repo) => repo.len(),
                        Self::Fake(ref repo) => repo.len(),
                    }
                }

                fn is_empty(&self) -> bool {
                    match self {
                        Self::Ebuild(ref repo) => repo.is_empty(),
                        Self::Fake(ref repo) => repo.is_empty(),
                    }
                }
            }
        )*
    };
}
make_repo!(Repo, BorrowedRepo<'_>);

macro_rules! make_repo_traits {
    ($($x:ty),*) => {
        $(
            impl PartialEq for $x {
                fn eq(&self, other: &Self) -> bool {
                    self.id() == other.id()
                }
            }

            impl Eq for $x {}

            impl std::hash::Hash for $x {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    self.id().hash(state);
                }
            }

            impl PartialOrd for $x {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    self.id().partial_cmp(other.id())
                }
            }

            impl Ord for $x {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    self.id().cmp(other.id())
                }
            }
        )*
    };
}
pub(self) use make_repo_traits;

/// A repo contains a given object.
pub trait Contains<T> {
    fn contains(&self, obj: T) -> bool;
}

impl<T: AsRef<Path>> Contains<T> for Repo {
    fn contains(&self, path: T) -> bool {
        match self {
            Self::Ebuild(ref repo) => repo.contains(path),
            Self::Fake(ref repo) => repo.contains(path),
        }
    }
}

macro_rules! make_contains {
    ($($x:ty),*) => {$(
        impl Contains<$x> for Repo {
            fn contains(&self, obj: $x) -> bool {
                match self {
                    Self::Ebuild(ref repo) => repo.contains(obj),
                    Self::Fake(ref repo) => repo.contains(obj),
                }
            }
        }
    )*};
}
make_contains!(atom::Atom, &atom::Atom);

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::repo::{ebuild, fake};

    #[test]
    fn test_traits() {
        let t = ebuild::TempRepo::new("test", None::<&str>, None).unwrap();
        let e_repo = Repo::Ebuild(t.repo);
        let f_repo = Repo::Fake(fake::Repo::new("fake", 0, []).unwrap());
        assert!(&e_repo != &f_repo);
        assert!(&e_repo > &f_repo);

        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 2);

        let f_repo = Repo::Fake(fake::Repo::new("test", 0, []).unwrap());
        assert!(&e_repo == &f_repo);
        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 1);
    }
}
