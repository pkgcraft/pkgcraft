use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use enum_as_inner::EnumAsInner;
use indexmap::IndexMap;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::atom::Atom;
use crate::config::RepoConfig;
use crate::pkg::{Package, Pkg};
use crate::restrict::Restrict;
use crate::Error;

pub mod ebuild;
pub(crate) mod empty;
pub mod fake;
pub mod set;

/// Supported repo formats
#[repr(C)]
#[derive(EnumIter, EnumString, Display, Debug, Default, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum RepoFormat {
    #[default]
    Ebuild,
    Fake,
    Empty,
}

#[allow(clippy::large_enum_variant)]
#[derive(EnumAsInner, Debug, Clone)]
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

impl From<Arc<ebuild::Repo>> for Repo {
    fn from(repo: Arc<ebuild::Repo>) -> Self {
        Self::Ebuild(repo)
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
    /// Try to load a repo from a given path.
    pub fn from_path<P, S>(id: S, priority: i32, path: P) -> crate::Result<Self>
    where
        P: AsRef<Utf8Path>,
        S: AsRef<str>,
    {
        let path = path.as_ref();
        let id = id.as_ref();

        if !path.exists() {
            return Err(Error::InvalidValue(format!("nonexistent repo path: {path:?}")));
        }

        for format in RepoFormat::iter() {
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
        format: RepoFormat,
    ) -> crate::Result<Self> {
        use RepoFormat::*;
        match format {
            Ebuild => Ok(ebuild::Repo::from_path(id, priority, path)?.into()),
            Fake => Ok(fake::Repo::from_path(id, priority, path)?.into()),
            Empty => Ok(empty::Repo::from_path(id, priority, path)?.into()),
        }
    }

    pub(super) fn finalize(&self, existing_repos: &IndexMap<String, Repo>) -> crate::Result<()> {
        match self {
            Self::Ebuild(repo) => repo.finalize(existing_repos, Arc::downgrade(repo)),
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

pub trait PkgRepository:
    fmt::Debug + PartialEq + Eq + PartialOrd + Ord + Hash + for<'a> Contains<&'a Atom>
{
    type Pkg<'a>: Package
    where
        Self: 'a;

    type Iterator<'a>: Iterator<Item = Self::Pkg<'a>>
    where
        Self: 'a;

    type RestrictIterator<'a>: Iterator<Item = Self::Pkg<'a>>
    where
        Self: 'a;

    fn categories(&self) -> Vec<String>;
    fn packages(&self, cat: &str) -> Vec<String>;
    fn versions(&self, cat: &str, pkg: &str) -> Vec<String>;
    fn len(&self) -> usize {
        let mut count = 0;
        for c in self.categories() {
            for p in self.packages(&c) {
                count += self.versions(&c, &p).len();
            }
        }
        count
    }
    fn iter(&self) -> Self::Iterator<'_>;
    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::RestrictIterator<'_>;

    fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

pub trait Repository: PkgRepository + fmt::Display {
    fn format(&self) -> RepoFormat;
    fn id(&self) -> &str;
    fn priority(&self) -> i32;
    fn path(&self) -> &Utf8Path;
    fn sync(&self) -> crate::Result<()>;
}

impl<'a, T> PkgRepository for &'a T
where
    T: PkgRepository,
{
    type Pkg<'b> = T::Pkg<'b> where Self: 'b;
    type Iterator<'b> = T::Iterator<'b> where Self: 'b;
    type RestrictIterator<'b> = T::RestrictIterator<'b> where Self: 'b;

    fn categories(&self) -> Vec<String> {
        (*self).categories()
    }
    fn packages(&self, cat: &str) -> Vec<String> {
        (*self).packages(cat)
    }
    fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
        (*self).versions(cat, pkg)
    }
    fn len(&self) -> usize {
        (*self).len()
    }
    fn iter(&self) -> Self::Iterator<'_> {
        (*self).iter()
    }
    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::RestrictIterator<'_> {
        (*self).iter_restrict(val)
    }
}

impl<T> Contains<&Atom> for &T
where
    T: PkgRepository,
{
    fn contains(&self, atom: &Atom) -> bool {
        self.iter_restrict(atom).next().is_some()
    }
}

impl<'a, T: Repository + PkgRepository> Repository for &'a T {
    fn format(&self) -> RepoFormat {
        (*self).format()
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

impl PkgRepository for Repo {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type Iterator<'a> = PkgIter<'a> where Self: 'a;
    type RestrictIterator<'a> = RestrictPkgIter<'a> where Self: 'a;

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

    fn len(&self) -> usize {
        match self {
            Self::Ebuild(repo) => repo.len(),
            Self::Fake(repo) => repo.len(),
            Self::Unsynced(repo) => repo.len(),
        }
    }

    fn iter(&self) -> Self::Iterator<'_> {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::RestrictIterator<'_> {
        match self {
            Self::Ebuild(repo) => RestrictPkgIter::Ebuild(repo.iter_restrict(val), self),
            Self::Fake(repo) => RestrictPkgIter::Fake(repo.iter_restrict(val), self),
            _ => RestrictPkgIter::Empty,
        }
    }
}

impl Repository for Repo {
    fn format(&self) -> RepoFormat {
        match self {
            Self::Ebuild(repo) => repo.format(),
            Self::Fake(repo) => repo.format(),
            Self::Unsynced(repo) => repo.format(),
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
            /// Order repos by priority then lexically by id.
            ///
            /// Note that priority comparisons are inverted so sorting returns higher priority
            /// repos before ones with lower priority.
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                use std::cmp::Ordering::*;
                match self.priority().cmp(&other.priority()) {
                    Less => Greater,
                    Greater => Less,
                    Equal => self.id().cmp(other.id()),
                }
            }
        }

        impl From<&$x> for crate::atom::Restrict {
            fn from(r: &$x) -> Self {
                crate::atom::Restrict::repo(Some(r.id()))
            }
        }

        impl From<&$x> for crate::pkg::Restrict {
            fn from(r: &$x) -> Self {
                crate::pkg::Restrict::repo(r.id())
            }
        }

        $crate::repo::make_contains_atom!($x);
        $crate::repo::make_contains_path!($x);
    )+};
}
pub(self) use make_repo_traits;

/// A repo contains a given object.
pub trait Contains<T> {
    fn contains(&self, obj: T) -> bool;
}

macro_rules! make_contains_atom {
    ($x:ty) => {
        impl $crate::repo::Contains<&crate::atom::Atom> for $x {
            fn contains(&self, atom: &crate::atom::Atom) -> bool {
                self.iter_restrict(atom).next().is_some()
            }
        }
    };
}
pub(self) use make_contains_atom;

macro_rules! make_contains_path {
    ($x:ty) => {
        impl<T: AsRef<Utf8Path>> $crate::repo::Contains<T> for $x {
            fn contains(&self, path: T) -> bool {
                match self.path() {
                    p if p.as_str().is_empty() => false,
                    repo_path => {
                        let path = path.as_ref();
                        if path.is_absolute() {
                            if let (Ok(path), Ok(repo_path)) =
                                (path.canonicalize(), repo_path.canonicalize())
                            {
                                path.starts_with(&repo_path) && path.exists()
                            } else {
                                false
                            }
                        } else {
                            repo_path.join(path).exists()
                        }
                    }
                }
            }
        }
    };
}
pub(self) use make_contains_path;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::repo::{ebuild, fake};

    use super::*;

    #[test]
    fn test_traits() {
        let t = ebuild::TempRepo::new("test", None, None).unwrap();
        let repo = ebuild::Repo::from_path("test", 0, t.path()).unwrap();
        let e_repo: Repo = repo.into();
        let f_repo: Repo = fake::Repo::new("fake", 0, []).into();
        assert!(e_repo != f_repo);
        assert!(e_repo > f_repo);

        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 2);

        let f_repo: Repo = fake::Repo::new("test", 0, []).into();
        assert!(e_repo == f_repo);
        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 1);
    }
}
