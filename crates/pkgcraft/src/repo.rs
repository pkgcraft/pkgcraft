use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use camino::Utf8Path;
use enum_as_inner::EnumAsInner;
use indexmap::{IndexMap, IndexSet};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};
use tracing::debug;

use crate::config::RepoConfig;
use crate::dep::{Cpn, Cpv, Dep, Version};
use crate::pkg::{Package, Pkg};
use crate::restrict::Restrict;
use crate::traits::Contains;
use crate::Error;

pub mod ebuild;
pub(crate) mod empty;
pub mod fake;
pub mod set;

/// Supported repo formats
#[repr(C)]
#[derive(
    EnumIter, EnumString, Display, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
#[strum(serialize_all = "snake_case")]
pub enum RepoFormat {
    #[default]
    Ebuild,
    Configured,
    Fake,
    Empty,
}

impl RepoFormat {
    /// Try to load a specific repo type from a given path.
    pub fn load_from_path<P: AsRef<Utf8Path>, S: AsRef<str>>(
        &self,
        id: S,
        path: P,
        priority: i32,
        finalize: bool,
    ) -> crate::Result<Repo> {
        let path = path.as_ref();
        let abspath = path.canonicalize_utf8().map_err(|e| Error::InvalidRepo {
            id: path.to_string(),
            err: e.to_string(),
        })?;

        // don't use relative paths for repo ids
        let mut id = id.as_ref();
        if id == path {
            id = abspath.as_str();
        }

        let repo: Repo = match self {
            Self::Ebuild => ebuild::Repo::from_path(id, priority, &abspath)?.into(),
            Self::Fake => fake::Repo::from_path(id, priority, &abspath)?.into(),
            Self::Empty => empty::Repo::from_path(id, priority, &abspath)?.into(),
            _ => {
                return Err(Error::LoadRepo {
                    kind: *self,
                    id: id.to_string(),
                })
            }
        };

        // try to finalize as a stand-alone repo
        if finalize {
            let existing = IndexMap::<_, _>::new();
            repo.finalize(&existing)
                .map_err(|e| Error::RepoInit(format!("overlay must be added via config: {e}")))?;
        }

        Ok(repo)
    }

    /// Try to load a specific repo type from a given path, traversing parents.
    pub fn load_from_nested_path<P: AsRef<Utf8Path>>(
        self,
        path: P,
        priority: i32,
        finalize: bool,
    ) -> crate::Result<Repo> {
        let path = path.as_ref();
        let abspath = path.canonicalize_utf8().map_err(|e| Error::InvalidRepo {
            id: path.to_string(),
            err: e.to_string(),
        })?;

        let mut path = abspath.as_path();
        while let Some(parent) = path.parent() {
            match self.load_from_path(path, path, priority, finalize) {
                Err(Error::NotARepo { .. }) => path = parent,
                result => return result,
            }
        }

        Err(Error::NotARepo {
            kind: self,
            id: abspath.to_string(),
            err: "no nested repo found".to_string(),
        })
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(EnumAsInner, Debug, Clone)]
pub enum Repo {
    Configured(Arc<ebuild::configured::Repo>),
    Ebuild(Arc<ebuild::Repo>),
    Fake(Arc<fake::Repo>),
    Unsynced(Arc<empty::Repo>),
}

impl From<&Repo> for Repo {
    fn from(repo: &Repo) -> Self {
        repo.clone()
    }
}

impl From<ebuild::Repo> for Repo {
    fn from(repo: ebuild::Repo) -> Self {
        Self::Ebuild(Arc::new(repo))
    }
}

impl From<ebuild::configured::Repo> for Repo {
    fn from(repo: ebuild::configured::Repo) -> Self {
        Self::Configured(Arc::new(repo))
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

impl From<&Repo> for Restrict {
    fn from(repo: &Repo) -> Self {
        repo.restrict_from_path(repo.path()).unwrap_or(Self::False)
    }
}

impl PartialEq for Repo {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ebuild(r1), Self::Ebuild(r2)) => r1.eq(r2),
            (Self::Configured(r1), Self::Configured(r2)) => r1.eq(r2),
            (Self::Fake(r1), Self::Fake(r2)) => r1.eq(r2),
            (Self::Unsynced(r1), Self::Unsynced(r2)) => r1.eq(r2),
            // list unmatched formats for compile failure visibility when adding types
            (Self::Ebuild(_), _) => false,
            (Self::Configured(_), _) => false,
            (Self::Fake(_), _) => false,
            (Self::Unsynced(_), _) => false,
        }
    }
}

impl Eq for Repo {}

impl Hash for Repo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.format().hash(state);
        match self {
            Self::Ebuild(r) => r.hash(state),
            Self::Configured(r) => r.hash(state),
            Self::Fake(r) => r.hash(state),
            Self::Unsynced(r) => r.hash(state),
        }
    }
}

make_repo_traits!(Repo);

impl Repo {
    /// Try to load a repo from a path.
    pub fn from_path<S: AsRef<str>, P: AsRef<Utf8Path>>(
        id: S,
        path: P,
        priority: i32,
        finalize: bool,
    ) -> crate::Result<Self> {
        let id = id.as_ref();
        let path = path.as_ref();

        for format in RepoFormat::iter() {
            match format.load_from_path(id, path, priority, finalize) {
                Err(e @ Error::NotARepo { .. }) => debug!("{e}"),
                Err(Error::LoadRepo { .. }) => (),
                Err(e) => return Err(e),
                result => return result,
            }
        }

        let err = if path.exists() {
            "unknown or invalid format"
        } else {
            "nonexistent repo path"
        };

        Err(Error::RepoInit(err.to_string()))
    }

    /// Try to load a repo from a potentially nested path.
    pub fn from_nested_path<P: AsRef<Utf8Path>>(
        path: P,
        priority: i32,
        finalize: bool,
    ) -> crate::Result<Self> {
        let path = path.as_ref();

        for format in RepoFormat::iter() {
            match format.load_from_nested_path(path, priority, finalize) {
                Err(e @ Error::NotARepo { .. }) => debug!("{e}"),
                Err(Error::LoadRepo { .. }) => (),
                Err(e) => return Err(e),
                result => return result,
            }
        }

        let err = if path.exists() {
            "unknown or invalid format"
        } else {
            "nonexistent repo path"
        };

        Err(Error::RepoInit(err.to_string()))
    }

    pub(super) fn finalize(&self, existing_repos: &IndexMap<String, Repo>) -> crate::Result<()> {
        match self {
            Self::Ebuild(repo) => repo.finalize(existing_repos, Arc::downgrade(repo)),
            _ => Ok(()),
        }
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        match self {
            Self::Configured(repo) => repo.repo_config(),
            Self::Ebuild(repo) => repo.repo_config(),
            Self::Fake(repo) => repo.repo_config(),
            Self::Unsynced(repo) => repo.repo_config(),
        }
    }
}

pub enum IterCpv<'a> {
    Configured(ebuild::IterCpv<'a>),
    Ebuild(ebuild::IterCpv<'a>),
    Fake(fake::IterCpv<'a>),
    Empty,
}

impl Iterator for IterCpv<'_> {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Configured(iter) => iter.next(),
            Self::Ebuild(iter) => iter.next(),
            Self::Fake(iter) => iter.next(),
            Self::Empty => None,
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum Iter<'a> {
    Ebuild(ebuild::Iter<'a>, &'a Repo),
    Configured(ebuild::configured::Iter<'a>, &'a Repo),
    Fake(fake::Iter<'a>, &'a Repo),
    Empty,
}

impl<'a> IntoIterator for &'a Repo {
    type Item = Pkg<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Repo::Ebuild(repo) => Iter::Ebuild(repo.into_iter(), self),
            Repo::Configured(repo) => Iter::Configured(repo.into_iter(), self),
            Repo::Fake(repo) => Iter::Fake(repo.into_iter(), self),
            Repo::Unsynced(_) => Iter::Empty,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ebuild(iter, repo) => iter.next().map(|p| Pkg::Ebuild(p, repo)),
            Self::Configured(iter, repo) => iter.next().map(|p| Pkg::Configured(p, repo)),
            Self::Fake(iter, repo) => iter.next().map(|p| Pkg::Fake(p, repo)),
            Self::Empty => None,
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum IterRestrict<'a> {
    Configured(ebuild::configured::IterRestrict<'a>, &'a Repo),
    Ebuild(ebuild::IterRestrict<'a>, &'a Repo),
    Fake(fake::IterRestrict<'a>, &'a Repo),
    Empty,
}

impl<'a> Iterator for IterRestrict<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Configured(iter, repo) => iter.next().map(|p| Pkg::Configured(p, repo)),
            Self::Ebuild(iter, repo) => iter.next().map(|p| Pkg::Ebuild(p, repo)),
            Self::Fake(iter, repo) => iter.next().map(|p| Pkg::Fake(p, repo)),
            Self::Empty => None,
        }
    }
}

pub trait PkgRepository:
    fmt::Debug
    + Ord
    + Hash
    + for<'a> Contains<&'a Cpn>
    + for<'a> Contains<&'a Cpv>
    + for<'a> Contains<&'a Dep>
{
    type Pkg<'a>: Package
    where
        Self: 'a;

    type IterCpv<'a>: Iterator<Item = Cpv>
    where
        Self: 'a;

    type Iter<'a>: Iterator<Item = Self::Pkg<'a>>
    where
        Self: 'a;

    type IterRestrict<'a>: Iterator<Item = Self::Pkg<'a>>
    where
        Self: 'a;

    fn categories(&self) -> IndexSet<String>;
    fn packages(&self, cat: &str) -> IndexSet<String>;
    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version>;
    fn len(&self) -> usize {
        let mut count = 0;
        for c in self.categories() {
            for p in self.packages(&c) {
                count += self.versions(&c, &p).len();
            }
        }
        count
    }
    fn iter_cpv(&self) -> Self::IterCpv<'_>;
    fn iter(&self) -> Self::Iter<'_>;
    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict<'_>;

    fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

pub trait Repository: PkgRepository + fmt::Display {
    fn format(&self) -> RepoFormat;
    /// Locally configured repo identifier.
    fn id(&self) -> &str;
    /// Official repo identifier.
    fn name(&self) -> &str {
        self.id()
    }
    fn priority(&self) -> i32;
    fn path(&self) -> &Utf8Path;
    /// Try converting a path to a [`Restrict`], returns None if the path isn't in the repo.
    fn restrict_from_path<P: AsRef<Utf8Path>>(&self, _path: P) -> Option<Restrict> {
        None
    }
    fn sync(&self) -> crate::Result<()>;
}

impl<'a, T> PkgRepository for &'a T
where
    T: PkgRepository,
{
    type Pkg<'b> = T::Pkg<'b> where Self: 'b;
    type IterCpv<'b> = T::IterCpv<'b> where Self: 'b;
    type Iter<'b> = T::Iter<'b> where Self: 'b;
    type IterRestrict<'b> = T::IterRestrict<'b> where Self: 'b;

    fn categories(&self) -> IndexSet<String> {
        (*self).categories()
    }
    fn packages(&self, cat: &str) -> IndexSet<String> {
        (*self).packages(cat)
    }
    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        (*self).versions(cat, pkg)
    }
    fn len(&self) -> usize {
        (*self).len()
    }
    fn iter_cpv(&self) -> Self::IterCpv<'_> {
        (*self).iter_cpv()
    }
    fn iter(&self) -> Self::Iter<'_> {
        (*self).iter()
    }
    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict<'_> {
        (*self).iter_restrict(val)
    }
}

impl<'a, T: Repository + PkgRepository> Repository for &'a T {
    fn format(&self) -> RepoFormat {
        (*self).format()
    }
    fn id(&self) -> &str {
        (*self).id()
    }
    fn name(&self) -> &str {
        (*self).name()
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
            Self::Configured(repo) => write!(f, "{repo}"),
            Self::Ebuild(repo) => write!(f, "{repo}"),
            Self::Fake(repo) => write!(f, "{repo}"),
            Self::Unsynced(repo) => write!(f, "{repo}"),
        }
    }
}

impl PkgRepository for Repo {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type IterCpv<'a> = IterCpv<'a> where Self: 'a;
    type Iter<'a> = Iter<'a> where Self: 'a;
    type IterRestrict<'a> = IterRestrict<'a> where Self: 'a;

    fn categories(&self) -> IndexSet<String> {
        match self {
            Self::Configured(repo) => repo.categories(),
            Self::Ebuild(repo) => repo.categories(),
            Self::Fake(repo) => repo.categories(),
            Self::Unsynced(repo) => repo.categories(),
        }
    }

    fn packages(&self, cat: &str) -> IndexSet<String> {
        match self {
            Self::Configured(repo) => repo.packages(cat),
            Self::Ebuild(repo) => repo.packages(cat),
            Self::Fake(repo) => repo.packages(cat),
            Self::Unsynced(repo) => repo.packages(cat),
        }
    }

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        match self {
            Self::Configured(repo) => repo.versions(cat, pkg),
            Self::Ebuild(repo) => repo.versions(cat, pkg),
            Self::Fake(repo) => repo.versions(cat, pkg),
            Self::Unsynced(repo) => repo.versions(cat, pkg),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Configured(repo) => repo.len(),
            Self::Ebuild(repo) => repo.len(),
            Self::Fake(repo) => repo.len(),
            Self::Unsynced(repo) => repo.len(),
        }
    }

    fn iter_cpv(&self) -> Self::IterCpv<'_> {
        match self {
            Self::Configured(repo) => IterCpv::Ebuild(repo.iter_cpv()),
            Self::Ebuild(repo) => IterCpv::Ebuild(repo.iter_cpv()),
            Self::Fake(repo) => IterCpv::Fake(repo.iter_cpv()),
            Self::Unsynced(_) => IterCpv::Empty,
        }
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict<'_> {
        match self {
            Self::Configured(repo) => IterRestrict::Configured(repo.iter_restrict(val), self),
            Self::Ebuild(repo) => IterRestrict::Ebuild(repo.iter_restrict(val), self),
            Self::Fake(repo) => IterRestrict::Fake(repo.iter_restrict(val), self),
            Self::Unsynced(_) => IterRestrict::Empty,
        }
    }
}

impl Contains<&Cpn> for Repo {
    fn contains(&self, value: &Cpn) -> bool {
        match self {
            Self::Configured(repo) => repo.contains(value),
            Self::Ebuild(repo) => repo.contains(value),
            Self::Fake(repo) => repo.contains(value),
            Self::Unsynced(repo) => repo.contains(value),
        }
    }
}

impl Contains<&Cpv> for Repo {
    fn contains(&self, value: &Cpv) -> bool {
        match self {
            Self::Configured(repo) => repo.contains(value),
            Self::Ebuild(repo) => repo.contains(value),
            Self::Fake(repo) => repo.contains(value),
            Self::Unsynced(repo) => repo.contains(value),
        }
    }
}

impl Contains<&Dep> for Repo {
    fn contains(&self, value: &Dep) -> bool {
        match self {
            Self::Configured(repo) => repo.contains(value),
            Self::Ebuild(repo) => repo.contains(value),
            Self::Fake(repo) => repo.contains(value),
            Self::Unsynced(repo) => repo.contains(value),
        }
    }
}

impl Repository for Repo {
    fn format(&self) -> RepoFormat {
        match self {
            Self::Configured(repo) => repo.format(),
            Self::Ebuild(repo) => repo.format(),
            Self::Fake(repo) => repo.format(),
            Self::Unsynced(repo) => repo.format(),
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Configured(repo) => repo.id(),
            Self::Ebuild(repo) => repo.id(),
            Self::Fake(repo) => repo.id(),
            Self::Unsynced(repo) => repo.id(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Configured(repo) => repo.name(),
            Self::Ebuild(repo) => repo.name(),
            Self::Fake(repo) => repo.id(),
            Self::Unsynced(repo) => repo.id(),
        }
    }

    fn priority(&self) -> i32 {
        match self {
            Self::Configured(repo) => repo.priority(),
            Self::Ebuild(repo) => repo.priority(),
            Self::Fake(repo) => repo.priority(),
            Self::Unsynced(repo) => repo.priority(),
        }
    }

    fn path(&self) -> &Utf8Path {
        match self {
            Self::Configured(repo) => repo.path(),
            Self::Ebuild(repo) => repo.path(),
            Self::Fake(repo) => repo.path(),
            Self::Unsynced(repo) => repo.path(),
        }
    }

    fn restrict_from_path<P: AsRef<Utf8Path>>(&self, path: P) -> Option<Restrict> {
        match self {
            Self::Configured(repo) => repo.restrict_from_path(path),
            Self::Ebuild(repo) => repo.restrict_from_path(path),
            Self::Fake(repo) => repo.restrict_from_path(path),
            Self::Unsynced(repo) => repo.restrict_from_path(path),
        }
    }

    fn sync(&self) -> crate::Result<()> {
        match self {
            Self::Configured(repo) => repo.sync(),
            Self::Ebuild(repo) => repo.sync(),
            Self::Fake(repo) => repo.sync(),
            Self::Unsynced(repo) => repo.sync(),
        }
    }
}

macro_rules! make_repo_traits {
    ($($x:ty),+) => {$(
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
                    Equal => {
                        self.id().cmp(other.id()).then_with(|| self.format().cmp(&other.format()))
                    }
                }
            }
        }

        impl AsRef<Utf8Path> for $x {
            fn as_ref(&self) -> &Utf8Path {
                self.path()
            }
        }

        impl AsRef<std::ffi::OsStr> for $x {
            fn as_ref(&self) -> &std::ffi::OsStr {
                self.path().as_ref()
            }
        }

        impl From<&$x> for crate::restrict::dep::Restrict {
            fn from(r: &$x) -> Self {
                crate::restrict::dep::Restrict::repo(Some(r.id()))
            }
        }

        impl From<&$x> for crate::pkg::Restrict {
            fn from(r: &$x) -> Self {
                crate::pkg::Restrict::repo(r.id())
            }
        }

        $crate::repo::make_contains_path!($x);
    )+};
}
use make_repo_traits;

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
use make_contains_path;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn traits() {
        let ebuild_repo = ebuild::temp::Repo::new("test", None, 0, None).unwrap();
        let e_repo = (&ebuild_repo).into();
        let f_repo: Repo = fake::Repo::new("fake", 0).into();
        assert!(e_repo != f_repo);
        assert!(e_repo > f_repo);

        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 2);

        let f_repo: Repo = fake::Repo::new("test", 0).into();
        assert!(e_repo != f_repo);
        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 2);
    }
}
