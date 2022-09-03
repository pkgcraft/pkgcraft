use std::fmt;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use enum_as_inner::EnumAsInner;
use indexmap::{IndexMap, IndexSet};
use once_cell::sync::Lazy;
use strum::{EnumIter, IntoEnumIterator, IntoStaticStr};
use tracing::warn;

use crate::config::RepoConfig;
use crate::pkg::{Package, Pkg};
use crate::restrict::{Restrict, Restriction};
use crate::{atom, Error};

pub mod ebuild;
pub(crate) mod empty;
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

#[derive(Debug)]
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
            match atom::cpv(s) {
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
#[derive(IntoStaticStr, EnumIter, EnumAsInner, Debug, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum Repo {
    Ebuild(Arc<ebuild::Repo>),
    Fake(Arc<fake::Repo>),
    Unsynced(Arc<empty::Repo>),
}

impl From<ebuild::Repo> for Repo {
    fn from(repo: ebuild::Repo) -> Self {
        Self::Ebuild(Arc::new(repo))
    }
}

impl From<fake::Repo> for Repo {
    fn from(repo: fake::Repo) -> Self {
        Self::Fake(Arc::new(repo))
    }
}

impl From<empty::Repo> for Repo {
    fn from(repo: empty::Repo) -> Self {
        Self::Unsynced(Arc::new(repo))
    }
}

make_repo_traits!(Repo);

impl Repo {
    /// Determine if a given repo format is supported.
    pub(crate) fn is_supported<S: AsRef<str>>(format: S) -> crate::Result<()> {
        let format = format.as_ref();
        match SUPPORTED_FORMATS.get(format) {
            Some(_) => Ok(()),
            None => Err(Error::RepoInit(format!("unknown repo format: {format}"))),
        }
    }

    /// Try to load a repo from a given path.
    pub(crate) fn from_path<P, S>(id: S, priority: i32, path: P) -> crate::Result<Self>
    where
        P: AsRef<Utf8Path>,
        S: AsRef<str>,
    {
        let path = path.as_ref();
        let id = id.as_ref();

        for format in SUPPORTED_FORMATS.iter() {
            if let Ok(repo) = Self::from_format(id, priority, path, format) {
                return Ok(repo);
            }
        }

        Err(Error::InvalidRepo {
            path: Utf8PathBuf::from(path),
            err: "unknown or invalid format".to_string(),
        })
    }

    /// Try to load a certain repo type from a given path.
    pub(crate) fn from_format(
        id: &str,
        priority: i32,
        path: &Utf8Path,
        format: &str,
    ) -> crate::Result<Self> {
        match format {
            "ebuild" => Ok(ebuild::Repo::from_path(id, priority, path)?.into()),
            "fake" => Ok(fake::Repo::from_path(id, priority, path)?.into()),
            "config" => Ok(empty::Repo::from_path(id, priority, path)?.into()),
            _ => Err(Error::RepoInit(format!("{id} repo: unknown format: {format}"))),
        }
    }

    pub(super) fn finalize(&self) -> crate::Result<()> {
        match self {
            Self::Ebuild(repo) => repo.finalize(),
            _ => Ok(()),
        }
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        match self {
            Self::Ebuild(repo) => repo.repo_config(),
            Self::Fake(repo) => repo.repo_config(),
            Self::Unsynced(repo) => repo.repo_config(),
        }
    }

    pub fn iter(&self) -> PkgIter {
        self.into_iter()
    }

    pub fn iter_restrict<T: Into<Restrict>>(&self, val: T) -> RestrictPkgIter {
        match self {
            Self::Ebuild(repo) => RestrictPkgIter::Ebuild(repo.iter_restrict(val), self),
            Self::Fake(repo) => RestrictPkgIter::Fake(repo.iter_restrict(val), self),
            _ => RestrictPkgIter::Empty,
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum PkgIter<'a> {
    Ebuild(ebuild::PkgIter<'a>, &'a Repo),
    Fake(fake::PkgIter<'a>, &'a Repo),
    Empty,
}

impl<'a> IntoIterator for &'a Repo {
    type Item = Pkg<'a>;
    type IntoIter = PkgIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Repo::Ebuild(repo) => PkgIter::Ebuild(repo.into_iter(), self),
            Repo::Fake(repo) => PkgIter::Fake(repo.into_iter(), self),
            _ => PkgIter::Empty,
        }
    }
}

impl<'a> Iterator for PkgIter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ebuild(iter, repo) => iter.next().map(|p| Pkg::Ebuild(p, repo)),
            Self::Fake(iter, repo) => iter.next().map(|p| Pkg::Fake(p, repo)),
            Self::Empty => None,
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum RestrictPkgIter<'a> {
    Ebuild(ebuild::RestrictPkgIter<'a>, &'a Repo),
    Fake(fake::RestrictPkgIter<'a>, &'a Repo),
    Empty,
}

impl<'a> Iterator for RestrictPkgIter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ebuild(iter, repo) => iter.next().map(|p| Pkg::Ebuild(p, repo)),
            Self::Fake(iter, repo) => iter.next().map(|p| Pkg::Fake(p, repo)),
            Self::Empty => None,
        }
    }
}

// externally supported repo formats
#[rustfmt::skip]
static SUPPORTED_FORMATS: Lazy<IndexSet<&'static str>> = Lazy::new(|| {
    <Repo as IntoEnumIterator>::iter().map(|r| r.into()).collect()
});

pub trait Repository: fmt::Debug + fmt::Display + PartialEq + Eq + PartialOrd + Ord {
    // TODO: add Iterator type and iter() when GATs are stabilized
    // https://github.com/rust-lang/rust/issues/44265
    fn categories(&self) -> Vec<String>;
    fn packages(&self, cat: &str) -> Vec<String>;
    fn versions(&self, cat: &str, pkg: &str) -> Vec<String>;
    fn id(&self) -> &str;
    fn priority(&self) -> i32;
    fn path(&self) -> &Utf8Path;
    fn sync(&self) -> crate::Result<()>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

impl<'a, T> Repository for &'a T
where
    T: Repository,
{
    fn categories(&self) -> Vec<String> {
        (*self).categories()
    }
    fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
        (*self).versions(cat, pkg)
    }
    fn id(&self) -> &str {
        (*self).id()
    }
    fn priority(&self) -> i32 {
        (*self).priority()
    }
    fn path(&self) -> &Utf8Path {
        (*self).path()
    }
    fn sync(&self) -> crate::Result<()> {
        (*self).sync()
    }
    fn packages(&self, cat: &str) -> Vec<String> {
        (*self).packages(cat)
    }
    fn len(&self) -> usize {
        (*self).len()
    }
    fn is_empty(&self) -> bool {
        (*self).is_empty()
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Ebuild(repo) => write!(f, "{}", repo),
            Self::Fake(repo) => write!(f, "{}", repo),
            Self::Unsynced(repo) => write!(f, "{}", repo),
        }
    }
}

impl Repository for Repo {
    fn categories(&self) -> Vec<String> {
        match self {
            Self::Ebuild(repo) => repo.categories(),
            Self::Fake(repo) => repo.categories(),
            Self::Unsynced(repo) => repo.categories(),
        }
    }

    fn packages(&self, cat: &str) -> Vec<String> {
        match self {
            Self::Ebuild(repo) => repo.packages(cat),
            Self::Fake(repo) => repo.packages(cat),
            Self::Unsynced(repo) => repo.packages(cat),
        }
    }

    fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
        match self {
            Self::Ebuild(repo) => repo.versions(cat, pkg),
            Self::Fake(repo) => repo.versions(cat, pkg),
            Self::Unsynced(repo) => repo.versions(cat, pkg),
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Ebuild(repo) => repo.id(),
            Self::Fake(repo) => repo.id(),
            Self::Unsynced(repo) => repo.id(),
        }
    }

    fn priority(&self) -> i32 {
        match self {
            Self::Ebuild(repo) => repo.priority(),
            Self::Fake(repo) => repo.priority(),
            Self::Unsynced(repo) => repo.priority(),
        }
    }

    fn path(&self) -> &Utf8Path {
        match self {
            Self::Ebuild(repo) => repo.path(),
            Self::Fake(repo) => repo.path(),
            Self::Unsynced(repo) => repo.path(),
        }
    }

    fn sync(&self) -> crate::Result<()> {
        match self {
            Self::Ebuild(repo) => repo.sync(),
            Self::Fake(repo) => repo.sync(),
            Self::Unsynced(repo) => repo.sync(),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Ebuild(repo) => repo.len(),
            Self::Fake(repo) => repo.len(),
            Self::Unsynced(repo) => repo.len(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Ebuild(repo) => repo.is_empty(),
            Self::Fake(repo) => repo.is_empty(),
            Self::Unsynced(repo) => repo.is_empty(),
        }
    }
}

macro_rules! make_repo_traits {
    ($($x:ty),+) => {$(
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
                Some(self.cmp(other))
            }
        }

        impl Ord for $x {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                $crate::macros::cmp_not_equal!(&self.priority(), &other.priority());
                self.id().cmp(other.id())
            }
        }

        $crate::repo::make_contains_atom!($x [atom::Atom, &atom::Atom]);
    )+};
}
pub(self) use make_repo_traits;

macro_rules! make_contains_atom {
    ($x:ty [$($y:ty),+]) => {$(
        impl $crate::repo::Contains<$y> for $x {
            fn contains(&self, atom: $y) -> bool {
                let r: $crate::restrict::Restrict = atom.into();
                self.iter().any(|p| r.matches(p.atom()))
            }
        }
    )+};
}
pub(self) use make_contains_atom;

/// A repo contains a given object.
pub trait Contains<T> {
    fn contains(&self, obj: T) -> bool;
}

impl<T: AsRef<Utf8Path>> Contains<T> for Repo {
    fn contains(&self, path: T) -> bool {
        match self {
            Self::Ebuild(repo) => repo.contains(path),
            Self::Fake(repo) => repo.contains(path),
            Self::Unsynced(repo) => repo.contains(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::repo::{ebuild, fake};

    #[test]
    fn test_traits() {
        let t = ebuild::TempRepo::new("test", None, None).unwrap();
        let repo = ebuild::Repo::from_path("test", 0, t.path).unwrap();
        let e_repo: Repo = repo.into();
        let f_repo: Repo = fake::Repo::new("fake", 0, []).unwrap().into();
        assert!(&e_repo != &f_repo);
        assert!(&e_repo > &f_repo);

        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 2);

        let f_repo: Repo = fake::Repo::new("test", 0, []).unwrap().into();
        assert!(&e_repo == &f_repo);
        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 1);
    }
}
