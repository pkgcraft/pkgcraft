use std::hash::{Hash, Hasher};
use std::{fmt, io};

use camino::{Utf8Path, Utf8PathBuf};
use enum_as_inner::EnumAsInner;
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};
use tracing::debug;

use crate::Error;
use crate::config::{Config, RepoConfig};
use crate::dep::{Cpn, Cpv, Dep, Version};
use crate::pkg::{Package, Pkg};
use crate::restrict::{Restrict, Restriction};
use crate::traits::Contains;

pub mod ebuild;
pub use ebuild::EbuildRepo;
pub mod fake;
pub use fake::FakeRepo;
pub mod set;

/// Supported repo formats
#[repr(C)]
#[derive(
    EnumIter,
    EnumString,
    Display,
    Deserialize,
    Serialize,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
)]
#[strum(serialize_all = "snake_case")]
pub enum RepoFormat {
    Ebuild,
    Configured,
    Fake,
}

impl RepoFormat {
    /// Try to load a specific repo type from a path.
    #[expect(clippy::wrong_self_convention)]
    fn from_config<S: AsRef<str>>(self, id: S, config: &RepoConfig) -> crate::Result<Repo> {
        let id = id.as_ref();
        if !config.location.exists() {
            return Err(Error::NonexistentRepo(config.location.to_string()));
        }

        match self {
            Self::Ebuild => Ok(EbuildRepo::from_config(id, config)?.into()),
            Self::Fake => Ok(FakeRepo::from_config(id, config)?.into()),
            _ => Err(Error::LoadRepo { kind: self, id: id.to_string() }),
        }
    }

    /// Try to load a specific repo type from a path.
    pub fn from_path<P: AsRef<Utf8Path>, S: AsRef<str>>(
        self,
        id: S,
        path: P,
        priority: i32,
    ) -> crate::Result<Repo> {
        let path = path.as_ref();
        let abspath = match path.canonicalize_utf8() {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(Error::InvalidRepo {
                id: path.to_string(),
                err: e.to_string(),
            }),
            Err(_) => Err(Error::NonexistentRepo(path.to_string())),
            Ok(path) => Ok(path),
        }?;

        // don't use relative paths for repo ids
        let mut id = id.as_ref();
        if id == path {
            id = abspath.as_str();
        }

        match self {
            Self::Ebuild => Ok(EbuildRepo::from_path(id, priority, &abspath)?.into()),
            Self::Fake => Ok(FakeRepo::from_path(id, priority, &abspath)?.into()),
            _ => Err(Error::LoadRepo { kind: self, id: id.to_string() }),
        }
    }

    /// Try to load a specific repo type from a path, traversing parents.
    pub fn from_nested_path<P: AsRef<Utf8Path>>(
        self,
        path: P,
        priority: i32,
    ) -> crate::Result<Repo> {
        let path = path.as_ref();
        let abspath = match path.canonicalize_utf8() {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(Error::InvalidRepo {
                id: path.to_string(),
                err: e.to_string(),
            }),
            Err(_) => Err(Error::NonexistentRepo(path.to_string())),
            Ok(path) => Ok(path),
        }?;

        let mut path = abspath.as_path();
        while let Some(parent) = path.parent() {
            match self.from_path(path, path, priority) {
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
    Configured(ebuild::configured::ConfiguredRepo),
    Ebuild(EbuildRepo),
    Fake(FakeRepo),
}

impl From<&Repo> for Repo {
    fn from(repo: &Repo) -> Self {
        repo.clone()
    }
}

impl From<EbuildRepo> for Repo {
    fn from(repo: EbuildRepo) -> Self {
        Self::Ebuild(repo)
    }
}

impl From<ebuild::configured::ConfiguredRepo> for Repo {
    fn from(repo: ebuild::configured::ConfiguredRepo) -> Self {
        Self::Configured(repo)
    }
}

impl From<FakeRepo> for Repo {
    fn from(repo: FakeRepo) -> Self {
        Self::Fake(repo)
    }
}

/// Try creating a repo from a path.
macro_rules! make_repo_from_path {
    ($($x:ty),+) => {$(
        impl TryFrom<$x> for Repo {
            type Error = Error;

            fn try_from(path: $x) -> crate::Result<Self> {
                Repo::from_nested_path(path, 0)
            }
        }
    )+};
}
make_repo_from_path!(&Utf8Path, Utf8PathBuf, &Utf8PathBuf);

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
            // list unmatched formats for compile failure visibility when adding types
            (Self::Ebuild(_), _) => false,
            (Self::Configured(_), _) => false,
            (Self::Fake(_), _) => false,
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
        }
    }
}

make_repo_traits!(Repo);

impl Repo {
    /// Try to load a repo from a RepoConfig.
    pub(crate) fn from_config<S: AsRef<str>>(
        id: S,
        config: &RepoConfig,
    ) -> crate::Result<Self> {
        config.format.from_config(id, config)
    }

    /// Try to load a repo from a path.
    pub fn from_path<S: AsRef<str>, P: AsRef<Utf8Path>>(
        id: S,
        path: P,
        priority: i32,
    ) -> crate::Result<Self> {
        let id = id.as_ref();
        let path = path.as_ref();

        for format in RepoFormat::iter() {
            match format.from_path(id, path, priority) {
                Err(e @ Error::NotARepo { .. }) => debug!("{e}"),
                Err(Error::LoadRepo { .. }) => (),
                result => return result,
            }
        }

        Err(Error::InvalidValue(format!("invalid repo: {path}")))
    }

    /// Try to load a repo from a potentially nested path.
    pub fn from_nested_path<P: AsRef<Utf8Path>>(
        path: P,
        priority: i32,
    ) -> crate::Result<Self> {
        let path = path.as_ref();

        for format in RepoFormat::iter() {
            match format.from_nested_path(path, priority) {
                Err(e @ Error::NotARepo { .. }) => debug!("{e}"),
                Err(Error::LoadRepo { .. }) => (),
                result => return result,
            }
        }

        Err(Error::InvalidValue(format!("invalid repo: {path}")))
    }

    /// Finalize the repo, resolving repo dependencies and collapsing lazy metadata.
    pub(crate) fn finalize(&self, config: &Config) -> crate::Result<()> {
        match self {
            Self::Ebuild(repo) => repo.finalize(config),
            _ => Ok(()),
        }
    }

    pub(crate) fn repo_config(&self) -> &RepoConfig {
        match self {
            Self::Configured(repo) => repo.repo_config(),
            Self::Ebuild(repo) => repo.repo_config(),
            Self::Fake(repo) => repo.repo_config(),
        }
    }
}

pub enum IterCpn {
    Configured(ebuild::IterCpn),
    Ebuild(ebuild::IterCpn),
    Fake(fake::IterCpn),
}

impl Iterator for IterCpn {
    type Item = Cpn;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Configured(iter) => iter.next(),
            Self::Ebuild(iter) => iter.next(),
            Self::Fake(iter) => iter.next(),
        }
    }
}

pub enum IterCpnRestrict {
    Configured(ebuild::IterCpnRestrict),
    Ebuild(ebuild::IterCpnRestrict),
    Fake(fake::IterCpnRestrict),
}

impl Iterator for IterCpnRestrict {
    type Item = Cpn;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Configured(iter) => iter.next(),
            Self::Ebuild(iter) => iter.next(),
            Self::Fake(iter) => iter.next(),
        }
    }
}

pub enum IterCpv {
    Configured(ebuild::IterCpv),
    Ebuild(ebuild::IterCpv),
    Fake(fake::IterCpv),
}

impl Iterator for IterCpv {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Configured(iter) => iter.next(),
            Self::Ebuild(iter) => iter.next(),
            Self::Fake(iter) => iter.next(),
        }
    }
}

pub enum IterCpvRestrict {
    Configured(ebuild::IterCpvRestrict),
    Ebuild(ebuild::IterCpvRestrict),
    Fake(fake::IterCpvRestrict),
}

impl Iterator for IterCpvRestrict {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Configured(iter) => iter.next(),
            Self::Ebuild(iter) => iter.next(),
            Self::Fake(iter) => iter.next(),
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum Iter {
    Ebuild(ebuild::Iter),
    Configured(ebuild::configured::Iter),
    Fake(fake::Iter),
}

impl IntoIterator for &Repo {
    type Item = crate::Result<Pkg>;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Repo::Ebuild(repo) => Iter::Ebuild(repo.into_iter()),
            Repo::Configured(repo) => Iter::Configured(repo.into_iter()),
            Repo::Fake(repo) => Iter::Fake(repo.into_iter()),
        }
    }
}

impl Iterator for Iter {
    type Item = crate::Result<Pkg>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Ebuild(iter) => iter.next().map(|x| x.map(Pkg::Ebuild)),
            Self::Configured(iter) => iter.next().map(|x| x.map(Pkg::Configured)),
            Self::Fake(iter) => iter.next().map(|x| x.map(Pkg::Fake)),
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum IterRestrict {
    Configured(ebuild::configured::IterRestrict),
    Ebuild(ebuild::IterRestrictOrdered),
    Fake(fake::IterRestrict),
}

impl Iterator for IterRestrict {
    type Item = crate::Result<Pkg>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Configured(iter) => iter.next().map(|x| x.map(Pkg::Configured)),
            Self::Ebuild(iter) => iter.next().map(|x| x.map(Pkg::Ebuild)),
            Self::Fake(iter) => iter.next().map(|x| x.map(Pkg::Fake)),
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
    + for<'a> Contains<&'a Restrict>
{
    type Pkg: Package;
    type IterCpn: Iterator<Item = Cpn>;
    type IterCpnRestrict: Iterator<Item = Cpn>;
    type IterCpv: Iterator<Item = Cpv>;
    type IterCpvRestrict: Iterator<Item = Cpv>;
    type Iter: Iterator<Item = crate::Result<Self::Pkg>>;
    type IterRestrict: Iterator<Item = crate::Result<Self::Pkg>>;

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

    /// Return true if the repo doesn't have any packages.
    fn is_empty(&self) -> bool {
        self.iter_cpv().next().is_none()
    }

    /// Return an iterator of Cpns for the repo.
    fn iter_cpn(&self) -> Self::IterCpn;

    /// Return a filtered iterator of Cpns for the repo.
    fn iter_cpn_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpnRestrict;

    /// Return an iterator of Cpvs for the repo.
    fn iter_cpv(&self) -> Self::IterCpv;

    /// Return a filtered iterator of Cpvs for the repo.
    fn iter_cpv_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpvRestrict;

    /// Return an iterator of packages for the repo.
    fn iter(&self) -> Self::Iter;

    /// Return a filtered iterator of packages for the repo.
    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict;
}

pub trait Repository: PkgRepository + fmt::Display {
    /// Return the repo's format.
    fn format(&self) -> RepoFormat;

    /// Return the repo's locally configured identifier.
    fn id(&self) -> &str;

    /// Return the repo's official identifier.
    fn name(&self) -> &str {
        self.id()
    }

    /// Return the repo's priority.
    fn priority(&self) -> i32;

    /// Return the repo's path.
    fn path(&self) -> &Utf8Path;

    /// Try converting a path to a [`Restrict`], returns None if the path isn't in the repo.
    fn restrict_from_path<P: AsRef<Utf8Path>>(&self, _path: P) -> Option<Restrict> {
        None
    }

    /// Try to sync the repo.
    fn sync(&self) -> crate::Result<()>;
}

impl<T> PkgRepository for &T
where
    T: PkgRepository,
{
    type Pkg = T::Pkg;
    type IterCpn = T::IterCpn;
    type IterCpnRestrict = T::IterCpnRestrict;
    type IterCpv = T::IterCpv;
    type IterCpvRestrict = T::IterCpvRestrict;
    type Iter = T::Iter;
    type IterRestrict = T::IterRestrict;

    fn categories(&self) -> IndexSet<String> {
        (*self).categories()
    }
    fn packages(&self, cat: &str) -> IndexSet<String> {
        (*self).packages(cat)
    }
    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        (*self).versions(cat, pkg)
    }
    fn iter_cpn(&self) -> Self::IterCpn {
        (*self).iter_cpn()
    }
    fn iter_cpn_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpnRestrict {
        (*self).iter_cpn_restrict(value)
    }
    fn iter_cpv(&self) -> Self::IterCpv {
        (*self).iter_cpv()
    }
    fn iter_cpv_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpvRestrict {
        (*self).iter_cpv_restrict(value)
    }
    fn iter(&self) -> Self::Iter {
        (*self).iter()
    }
    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict {
        (*self).iter_restrict(val)
    }
}

impl<T: Repository + PkgRepository> Repository for &T {
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
        }
    }
}

impl PkgRepository for Repo {
    type Pkg = Pkg;
    type IterCpn = IterCpn;
    type IterCpnRestrict = IterCpnRestrict;
    type IterCpv = IterCpv;
    type IterCpvRestrict = IterCpvRestrict;
    type Iter = Iter;
    type IterRestrict = IterRestrict;

    fn categories(&self) -> IndexSet<String> {
        match self {
            Self::Configured(repo) => repo.categories(),
            Self::Ebuild(repo) => repo.categories(),
            Self::Fake(repo) => repo.categories(),
        }
    }

    fn packages(&self, cat: &str) -> IndexSet<String> {
        match self {
            Self::Configured(repo) => repo.packages(cat),
            Self::Ebuild(repo) => repo.packages(cat),
            Self::Fake(repo) => repo.packages(cat),
        }
    }

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        match self {
            Self::Configured(repo) => repo.versions(cat, pkg),
            Self::Ebuild(repo) => repo.versions(cat, pkg),
            Self::Fake(repo) => repo.versions(cat, pkg),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Configured(repo) => repo.len(),
            Self::Ebuild(repo) => repo.len(),
            Self::Fake(repo) => repo.len(),
        }
    }

    fn iter_cpn(&self) -> Self::IterCpn {
        match self {
            Self::Configured(repo) => IterCpn::Ebuild(repo.iter_cpn()),
            Self::Ebuild(repo) => IterCpn::Ebuild(repo.iter_cpn()),
            Self::Fake(repo) => IterCpn::Fake(repo.iter_cpn()),
        }
    }

    fn iter_cpn_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpnRestrict {
        match self {
            Self::Configured(repo) => IterCpnRestrict::Ebuild(repo.iter_cpn_restrict(value)),
            Self::Ebuild(repo) => IterCpnRestrict::Ebuild(repo.iter_cpn_restrict(value)),
            Self::Fake(repo) => IterCpnRestrict::Fake(repo.iter_cpn_restrict(value)),
        }
    }

    fn iter_cpv(&self) -> Self::IterCpv {
        match self {
            Self::Configured(repo) => IterCpv::Ebuild(repo.iter_cpv()),
            Self::Ebuild(repo) => IterCpv::Ebuild(repo.iter_cpv()),
            Self::Fake(repo) => IterCpv::Fake(repo.iter_cpv()),
        }
    }

    fn iter_cpv_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpvRestrict {
        match self {
            Self::Configured(repo) => IterCpvRestrict::Ebuild(repo.iter_cpv_restrict(value)),
            Self::Ebuild(repo) => IterCpvRestrict::Ebuild(repo.iter_cpv_restrict(value)),
            Self::Fake(repo) => IterCpvRestrict::Fake(repo.iter_cpv_restrict(value)),
        }
    }

    fn iter(&self) -> Self::Iter {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict {
        match self {
            Self::Configured(repo) => IterRestrict::Configured(repo.iter_restrict(val)),
            Self::Ebuild(repo) => IterRestrict::Ebuild(repo.iter_restrict_ordered(val)),
            Self::Fake(repo) => IterRestrict::Fake(repo.iter_restrict(val)),
        }
    }
}

impl Contains<&Cpn> for Repo {
    fn contains(&self, value: &Cpn) -> bool {
        match self {
            Self::Configured(repo) => repo.contains(value),
            Self::Ebuild(repo) => repo.contains(value),
            Self::Fake(repo) => repo.contains(value),
        }
    }
}

impl Contains<&Cpv> for Repo {
    fn contains(&self, value: &Cpv) -> bool {
        match self {
            Self::Configured(repo) => repo.contains(value),
            Self::Ebuild(repo) => repo.contains(value),
            Self::Fake(repo) => repo.contains(value),
        }
    }
}

impl Contains<&Dep> for Repo {
    fn contains(&self, value: &Dep) -> bool {
        match self {
            Self::Configured(repo) => repo.contains(value),
            Self::Ebuild(repo) => repo.contains(value),
            Self::Fake(repo) => repo.contains(value),
        }
    }
}

impl Repository for Repo {
    fn format(&self) -> RepoFormat {
        match self {
            Self::Configured(repo) => repo.format(),
            Self::Ebuild(repo) => repo.format(),
            Self::Fake(repo) => repo.format(),
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Configured(repo) => repo.id(),
            Self::Ebuild(repo) => repo.id(),
            Self::Fake(repo) => repo.id(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Configured(repo) => repo.name(),
            Self::Ebuild(repo) => repo.name(),
            Self::Fake(repo) => repo.id(),
        }
    }

    fn priority(&self) -> i32 {
        match self {
            Self::Configured(repo) => repo.priority(),
            Self::Ebuild(repo) => repo.priority(),
            Self::Fake(repo) => repo.priority(),
        }
    }

    fn path(&self) -> &Utf8Path {
        match self {
            Self::Configured(repo) => repo.path(),
            Self::Ebuild(repo) => repo.path(),
            Self::Fake(repo) => repo.path(),
        }
    }

    fn restrict_from_path<P: AsRef<Utf8Path>>(&self, path: P) -> Option<Restrict> {
        match self {
            Self::Configured(repo) => repo.restrict_from_path(path),
            Self::Ebuild(repo) => repo.restrict_from_path(path),
            Self::Fake(repo) => repo.restrict_from_path(path),
        }
    }

    fn sync(&self) -> crate::Result<()> {
        match self {
            Self::Configured(repo) => repo.sync(),
            Self::Ebuild(repo) => repo.sync(),
            Self::Fake(repo) => repo.sync(),
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

        impl AsRef<camino::Utf8Path> for $x {
            fn as_ref(&self) -> &camino::Utf8Path {
                self.path()
            }
        }

        impl AsRef<std::path::Path> for $x {
            fn as_ref(&self) -> &std::path::Path {
                self.path().as_ref()
            }
        }

        impl AsRef<std::ffi::OsStr> for $x {
            fn as_ref(&self) -> &std::ffi::OsStr {
                self.path().as_ref()
            }
        }

        impl AsRef<str> for $x {
            fn as_ref(&self) -> &str {
                self.name()
            }
        }

        impl From<&$x> for crate::restrict::dep::Restrict {
            fn from(r: &$x) -> Self {
                crate::restrict::dep::Restrict::repo(Some(r.id()))
            }
        }

        impl From<$x> for crate::restrict::dep::Restrict {
            fn from(r: $x) -> Self {
                crate::restrict::dep::Restrict::repo(Some(r.id()))
            }
        }

        impl From<&$x> for crate::pkg::Restrict {
            fn from(r: &$x) -> Self {
                crate::pkg::Restrict::repo(r.id())
            }
        }

        impl From<$x> for crate::pkg::Restrict {
            fn from(r: $x) -> Self {
                crate::pkg::Restrict::repo(r.id())
            }
        }

        impl $crate::restrict::TryIntoRestrict<$x> for &camino::Utf8Path {
            fn try_into_restrict(self, repo: &$x) -> crate::Result<$crate::restrict::Restrict> {
                repo.restrict_from_path(self)
                    .ok_or_else(|| $crate::Error::InvalidValue(format!("{repo} repo doesn't contain path: {self}")))
            }
        }

        impl $crate::restrict::TryIntoRestrict<$x> for &camino::Utf8PathBuf {
            fn try_into_restrict(self, repo: &$x) -> crate::Result<$crate::restrict::Restrict> {
                let path: &Utf8Path = self.as_ref();
                path.try_into_restrict(repo)
            }
        }

        impl $crate::restrict::TryIntoRestrict<$x> for &$x {
            fn try_into_restrict(self, repo: &$x) -> crate::Result<$crate::restrict::Restrict> {
                let path: &Utf8Path = self.as_ref();
                path.try_into_restrict(repo)
            }
        }

        impl Contains<&crate::restrict::Restrict> for $x {
            fn contains(&self, value: &crate::restrict::Restrict) -> bool {
                use $crate::restrict::dep::Restrict as DepRestrict;
                use $crate::restrict::Restriction;
                match value {
                    Restrict::True => true,
                    Restrict::False => false,
                    Restrict::Dep(DepRestrict::Repo(Some(r))) => r.matches(self.id()),
                    _ => {
                        self.iter_cpv_restrict(value).next().is_some()
                            || self.iter_cpn_restrict(value).next().is_some()
                            || self.iter_restrict(value).next().is_some()
                    }
                }
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

impl Restriction<&Repo> for Restrict {
    fn matches(&self, repo: &Repo) -> bool {
        crate::restrict::restrict_match! {self, repo,
            Self::Dep(r) => r.matches(repo),
            Self::Pkg(r) => r.matches(repo),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use tempfile::tempdir;

    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::test::{assert_err_re, assert_ordered_eq};

    use super::*;

    #[test]
    fn traits() {
        let mut config = Config::default();

        // ebuild repo
        let mut temp = EbuildRepoBuilder::new().name("test").build().unwrap();
        let e_repo = config.add_repo(&temp).unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();

        // fake repo
        let fake = FakeRepo::new("fake", 0).pkgs(["cat/pkg-1"]).unwrap();
        let f_repo = config.add_repo(fake).unwrap();

        // finalize repos
        config.finalize().unwrap();

        // comparisons
        assert!(e_repo != f_repo);
        assert!(e_repo > f_repo);

        // equality
        let f_repo: Repo = FakeRepo::new("test", 0).into();
        assert!(e_repo != f_repo);
        assert!(f_repo != e_repo);
        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 2);

        // hash
        let repos: HashSet<_> = HashSet::from([&e_repo, &f_repo]);
        assert_eq!(repos.len(), 2);

        // PkgRepository for &PkgRepository type
        let repo = &&e_repo;
        let cpn = Cpn::try_new("cat/pkg").unwrap();
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        let pkg = repo.iter().next().unwrap().unwrap();
        assert_ordered_eq!(repo.categories(), ["cat"]);
        assert_ordered_eq!(repo.packages("cat"), ["pkg"]);
        assert_ordered_eq!(repo.versions("cat", "pkg").iter().map(|x| x.to_string()), ["1"]);
        assert_eq!(repo.len(), 1);
        assert_ordered_eq!(repo.iter_cpn(), [cpn.clone()]);
        assert_ordered_eq!(repo.iter_cpn_restrict(&cpn), [cpn.clone()]);
        assert_ordered_eq!(repo.iter_cpv(), [cpv.clone()]);
        assert_ordered_eq!(repo.iter_cpv_restrict(&cpv), [cpv.clone()]);
        assert_ordered_eq!(repo.iter(), [Ok(pkg.clone())]);
        assert_ordered_eq!(repo.iter_restrict(&pkg), [Ok(pkg.clone())]);
        // Repository for &PkgRepository type
        assert_eq!(repo.format(), RepoFormat::Ebuild);
        assert_eq!(repo.id(), "test");
        assert_eq!(repo.name(), "test");
        assert_eq!(repo.priority(), 0);
        assert_eq!(repo.path(), temp.path());
    }

    #[test]
    fn from_path() {
        let dir = tempdir().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();

        // invalid repo
        let r = Repo::from_path("test", path, 0);
        assert_err_re!(r, format!("^invalid repo: {path}$"));
    }

    #[test]
    fn from_nested_path() {
        let dir = tempdir().unwrap();
        let path = Utf8Path::from_path(dir.path()).unwrap();

        // invalid repo
        let r = Repo::from_nested_path(path, 0);
        assert_err_re!(r, format!("^invalid repo: {path}$"));

        // ebuild repo
        let mut temp = EbuildRepoBuilder::new().name("test").build().unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        let r = Repo::from_nested_path(temp.path().join("cat"), 0);
        assert!(r.is_ok());
        let r = Repo::from_nested_path(temp.path().join("cat/pkg"), 0);
        assert!(r.is_ok());
        let r = Repo::from_nested_path(temp.path().join("cat/pkg/pkg-1.ebuild"), 0);
        assert!(r.is_ok());
    }
}
