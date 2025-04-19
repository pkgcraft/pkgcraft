use std::fmt;

use enum_as_inner::EnumAsInner;
use scallop::ExecStatus;

use crate::dep::{Cpn, Cpv, Dep, Version};
use crate::eapi::{Eapi, Restrict as EapiRestrict};
use crate::repo::{Repo, Repository};
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::Intersects;

pub mod ebuild;
pub mod fake;

#[allow(clippy::large_enum_variant)]
#[derive(EnumAsInner, Debug, Clone)]
pub enum Pkg {
    Configured(ebuild::EbuildConfiguredPkg),
    Ebuild(ebuild::EbuildPkg),
    Fake(fake::Pkg),
}

make_pkg_traits!(Pkg);

pub trait Package:
    fmt::Debug + fmt::Display + Intersects<Dep> + Intersects<Cpv> + Intersects<Cpn>
{
    /// Return a package's EAPI.
    fn eapi(&self) -> &'static Eapi;

    /// Return a package's Cpv.
    fn cpv(&self) -> &Cpv;

    /// Return the unversioned package.
    fn cpn(&self) -> &Cpn {
        self.cpv().cpn()
    }

    /// Return a package's category.
    fn category(&self) -> &str {
        self.cpv().category()
    }

    /// Return a package's name.
    fn package(&self) -> &str {
        self.cpv().package()
    }

    /// Return a package's version.
    fn version(&self) -> &Version {
        self.cpv().version()
    }

    /// Return a package's name and version.
    fn p(&self) -> String {
        self.cpv().p()
    }

    /// Return a package's name, version, and revision.
    fn pf(&self) -> String {
        self.cpv().pf()
    }

    /// Return a package's revision.
    fn pr(&self) -> String {
        self.cpv().pr()
    }

    /// Return a package's version.
    fn pv(&self) -> String {
        self.cpv().pv()
    }

    /// Returna package's version and revision.
    fn pvr(&self) -> String {
        self.cpv().pvr()
    }
}

pub trait RepoPackage: Package + Ord {
    type Repo: Repository;

    /// Return a package's repo.
    fn repo(&self) -> Self::Repo;
}

pub(crate) trait Build: Package {
    /// Run the build operations for a package.
    fn build(&self) -> scallop::Result<()>;
}

pub(crate) trait PkgPretend: Package {
    /// Run the pkg_pretend operation for a package.
    fn pkg_pretend(&self) -> scallop::Result<Option<String>>;
}

pub trait Source: Package {
    /// Source a package.
    fn source(&self) -> scallop::Result<ExecStatus>;
}

macro_rules! make_pkg_traits {
    ($($x:ty),+) => {$(
        impl PartialEq for $x {
            fn eq(&self, other: &Self) -> bool {
                self.repo() == other.repo() && self.cpv() == other.cpv()
            }
        }

        impl Eq for $x {}

        impl std::hash::Hash for $x {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.repo().hash(state);
                self.cpv().hash(state);
            }
        }

        impl PartialOrd for $x {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $x {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.cpv().cmp(other.cpv()).then_with(|| self.repo().cmp(&other.repo()))
            }
        }

        impl std::fmt::Display for $x {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}::{}", self.cpv(), self.repo())
            }
        }

        impl From<&$x> for $crate::restrict::Restrict {
            fn from(pkg: &$x) -> Self {
                let r1 = pkg.cpv().into();
                let r2 = $crate::restrict::Restrict::Dep(pkg.repo().into());
                $crate::restrict::Restrict::and([r1, r2])
            }
        }

        impl $crate::traits::Intersects<$x> for $crate::dep::Dep {
            fn intersects(&self, other: &$x) -> bool {
                other.intersects(self)
            }
        }

        impl $crate::traits::Intersects<$crate::dep::Cpv> for $x {
            fn intersects(&self, cpv: &$crate::dep::Cpv) -> bool {
                self.cpv() == cpv
            }
        }

        impl $crate::traits::Intersects<$x> for $crate::dep::Cpv {
            fn intersects(&self, other: &$x) -> bool {
                other.intersects(self)
            }
        }

        impl $crate::traits::Intersects<$crate::dep::Cpn> for $x {
            fn intersects(&self, cpn: &$crate::dep::Cpn) -> bool {
                self.cpn() == cpn
            }
        }

        impl $crate::traits::Intersects<$x> for $crate::dep::Cpn {
            fn intersects(&self, other: &$x) -> bool {
                other.intersects(self)
            }
        }

        impl From<&$x> for $crate::dep::Cpv {
            fn from(value: &$x) -> Self {
                value.cpv().clone()
            }
        }
    )+};
}
use make_pkg_traits;

impl Package for Pkg {
    fn eapi(&self) -> &'static Eapi {
        match self {
            Self::Configured(pkg) => pkg.eapi(),
            Self::Ebuild(pkg) => pkg.eapi(),
            Self::Fake(pkg) => pkg.eapi(),
        }
    }

    fn cpv(&self) -> &Cpv {
        match self {
            Self::Configured(pkg) => pkg.cpv(),
            Self::Ebuild(pkg) => pkg.cpv(),
            Self::Fake(pkg) => pkg.cpv(),
        }
    }
}

impl RepoPackage for Pkg {
    type Repo = Repo;

    fn repo(&self) -> Self::Repo {
        match self {
            Self::Configured(pkg) => pkg.repo().into(),
            Self::Ebuild(pkg) => pkg.repo().into(),
            Self::Fake(pkg) => pkg.repo().into(),
        }
    }
}

impl Intersects<Dep> for Pkg {
    fn intersects(&self, dep: &Dep) -> bool {
        match self {
            Self::Configured(pkg) => pkg.intersects(dep),
            Self::Ebuild(pkg) => pkg.intersects(dep),
            Self::Fake(pkg) => pkg.intersects(dep),
        }
    }
}

impl<T> Package for &T
where
    T: Package,
{
    fn eapi(&self) -> &'static Eapi {
        (*self).eapi()
    }
    fn cpv(&self) -> &Cpv {
        (*self).cpv()
    }
}

impl<T> RepoPackage for &T
where
    T: RepoPackage,
{
    type Repo = T::Repo;

    fn repo(&self) -> Self::Repo {
        (*self).repo()
    }
}

impl<T> Intersects<Dep> for &T
where
    T: Package,
{
    fn intersects(&self, dep: &Dep) -> bool {
        (*self).intersects(dep)
    }
}

impl<T> Intersects<Cpv> for &T
where
    T: Package,
{
    fn intersects(&self, cpv: &Cpv) -> bool {
        (*self).intersects(cpv)
    }
}

impl<T> Intersects<Cpn> for &T
where
    T: Package,
{
    fn intersects(&self, cpn: &Cpn) -> bool {
        (*self).intersects(cpn)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict {
    Eapi(EapiRestrict),
    Ebuild(ebuild::Restrict),
    Repo(StrRestrict),
}

impl Restrict {
    pub fn eapi(s: &str) -> Self {
        Self::Eapi(EapiRestrict::Id(StrRestrict::equal(s)))
    }

    pub fn repo(s: &str) -> Self {
        Self::Repo(StrRestrict::equal(s))
    }
}

impl From<Restrict> for BaseRestrict {
    fn from(r: Restrict) -> Self {
        Self::Pkg(r)
    }
}

impl Restriction<&Pkg> for Restrict {
    fn matches(&self, pkg: &Pkg) -> bool {
        match self {
            Self::Eapi(r) => r.matches(pkg.eapi()),
            Self::Repo(r) => r.matches(pkg.repo().id()),
            Self::Ebuild(r) => match pkg {
                Pkg::Ebuild(p) => r.matches(p),
                _ => false,
            },
        }
    }
}

impl Restriction<&Pkg> for BaseRestrict {
    fn matches(&self, pkg: &Pkg) -> bool {
        crate::restrict::restrict_match! {self, pkg,
            Self::Dep(r) => r.matches(pkg),
            Self::Pkg(r) => r.matches(pkg),
        }
    }
}

impl Restriction<&Repo> for Restrict {
    fn matches(&self, repo: &Repo) -> bool {
        match self {
            Self::Repo(r) => r.matches(repo.id()),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::eapi::EAPI_LATEST_OFFICIAL;
    use crate::repo::{fake, PkgRepository};
    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn ordering() {
        // unmatching pkgs sorted by dep attributes
        let r1: Repo = fake::FakeRepo::new("b", 0)
            .pkgs(["cat/pkg-1"])
            .unwrap()
            .into();
        let r2: Repo = fake::FakeRepo::new("a", 0)
            .pkgs(["cat/pkg-0"])
            .unwrap()
            .into();
        let pkgs: Vec<_> = r1.iter().chain(r2.iter()).try_collect().unwrap();
        let sorted_pkgs: Vec<_> = pkgs.iter().sorted().collect();
        assert_ordered_eq!(pkgs.iter().rev(), sorted_pkgs);

        // matching pkgs sorted by repo priority
        let r1: Repo = fake::FakeRepo::new("a", -1)
            .pkgs(["cat/pkg-0"])
            .unwrap()
            .into();
        let r2: Repo = fake::FakeRepo::new("b", 0)
            .pkgs(["cat/pkg-0"])
            .unwrap()
            .into();
        let pkgs: Vec<_> = r1.iter().chain(r2.iter()).try_collect().unwrap();
        let sorted_pkgs: Vec<_> = pkgs.iter().sorted().collect();
        assert_ordered_eq!(pkgs.iter().rev(), sorted_pkgs);

        // matching pkgs sorted by repo id since repos have matching priorities
        let r1: Repo = fake::FakeRepo::new("2", 0)
            .pkgs(["cat/pkg-0"])
            .unwrap()
            .into();
        let r2: Repo = fake::FakeRepo::new("1", 0)
            .pkgs(["cat/pkg-0"])
            .unwrap()
            .into();
        let pkgs: Vec<_> = r1.iter().chain(r2.iter()).try_collect().unwrap();
        let sorted_pkgs: Vec<_> = pkgs.iter().sorted().collect();
        assert_ordered_eq!(pkgs.iter().rev(), sorted_pkgs);
    }

    #[test]
    fn package_trait() {
        let cpv = Cpv::try_new("cat/pkg-1-r2").unwrap();
        let r: Repo = fake::FakeRepo::new("test", 0).pkgs([&cpv]).unwrap().into();
        let pkg = r.iter_restrict(&cpv).next().unwrap().unwrap();
        assert_eq!(pkg.eapi(), *EAPI_LATEST_OFFICIAL);
        assert_eq!(pkg.cpv(), &cpv);
        assert_eq!(pkg.cpn().to_string(), "cat/pkg");
        assert_eq!(pkg.category(), "cat");
        assert_eq!(pkg.package(), "pkg");
        assert_eq!(pkg.version().to_string(), "1-r2");
        assert_eq!(pkg.p(), "pkg-1");
        assert_eq!(pkg.pf(), "pkg-1-r2");
        assert_eq!(pkg.pr(), "r2");
        assert_eq!(pkg.pv(), "1");
        assert_eq!(pkg.pvr(), "1-r2");
    }

    #[test]
    fn intersects_dep() {
        let cpv = Cpv::try_new("cat/pkg-1-r2").unwrap();
        let r: Repo = fake::FakeRepo::new("test", 0).pkgs([&cpv]).unwrap().into();
        let pkg = r.iter_restrict(&cpv).next().unwrap().unwrap();

        for (s, expected) in [
            ("cat/pkg", true),
            ("a/b", false),
            ("=cat/pkg-1-r2", true),
            (">cat/pkg-1-r2", false),
            ("~cat/pkg-1", true),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(pkg.intersects(&dep), expected, "failed for {s}");
        }
    }
}
