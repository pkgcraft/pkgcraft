use std::fmt;

use enum_as_inner::EnumAsInner;

use crate::dep::{Cpv, Version};
use crate::eapi::{self, Restrict as EapiRestrict};
use crate::repo::{Repo, Repository};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict as BaseRestrict, Restriction};

pub mod ebuild;
pub mod fake;

#[allow(clippy::large_enum_variant)]
#[derive(EnumAsInner, Debug)]
pub enum Pkg<'a> {
    Ebuild(ebuild::Pkg<'a>, &'a Repo),
    Fake(fake::Pkg<'a>, &'a Repo),
}

make_pkg_traits!(Pkg<'_>);

pub trait Package: fmt::Debug + fmt::Display + PartialEq + Eq + PartialOrd + Ord {
    type Repo: Repository;

    /// Return a package's EAPI.
    fn eapi(&self) -> &'static eapi::Eapi;

    /// Return a package's repo.
    fn repo(&self) -> Self::Repo;

    /// Return a package's CPV.
    fn cpv(&self) -> &Cpv;

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

    /// Return a package's category and package.
    fn cpn(&self) -> String {
        self.cpv().cpn()
    }
}

pub trait BuildablePackage: Package {
    /// Run the build operations for a package.
    fn build(&self) -> scallop::Result<()>;
    /// Run the pkg_pretend operation for a package.
    fn pretend(&self) -> scallop::Result<()>;
}

pub trait SourceablePackage: Package {
    /// Source a package.
    fn source(&self) -> scallop::Result<()>;
    /// Generate the metadata for a package.
    fn metadata(&self) -> scallop::Result<()>;
}

macro_rules! make_pkg_traits {
    ($($x:ty),+) => {$(
        impl crate::error::PackageError for $x {}

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
                crate::macros::cmp_not_equal!(self.cpv(), other.cpv());
                self.repo().cmp(&other.repo())
            }
        }

        impl std::fmt::Display for $x {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                use crate::repo::Repository;
                write!(f, "{}::{}", self.cpv(), self.repo().id())
            }
        }

        impl From<&$x> for crate::restrict::Restrict {
            fn from(pkg: &$x) -> Self {
                let r1 = pkg.cpv().into();
                let r2 = crate::restrict::Restrict::Dep(pkg.repo().into());
                crate::restrict::Restrict::and([r1, r2])
            }
        }
    )+};
}
pub(self) use make_pkg_traits;

impl<'a> Package for Pkg<'a> {
    type Repo = &'a Repo;

    fn cpv(&self) -> &Cpv {
        match self {
            Self::Ebuild(pkg, _) => pkg.cpv(),
            Self::Fake(pkg, _) => pkg.cpv(),
        }
    }

    fn eapi(&self) -> &'static eapi::Eapi {
        match self {
            Self::Ebuild(pkg, _) => pkg.eapi(),
            Self::Fake(pkg, _) => pkg.eapi(),
        }
    }

    fn repo(&self) -> Self::Repo {
        match self {
            Self::Ebuild(_, repo) => repo,
            Self::Fake(_, repo) => repo,
        }
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

impl<'a> Restriction<&'a Pkg<'a>> for Restrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use Restrict::*;
        match self {
            Eapi(r) => r.matches(pkg.eapi()),
            Repo(r) => r.matches(pkg.repo().id()),
            Ebuild(r) => match pkg {
                Pkg::Ebuild(p, _) => r.matches(p),
                _ => false,
            },
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for BaseRestrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use BaseRestrict::*;
        crate::restrict::restrict_match! {self, pkg,
            Dep(r) => r.matches(pkg),
            Pkg(r) => r.matches(pkg),
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for DepRestrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use DepRestrict::*;
        match self {
            Repo(Some(r)) => r.matches(pkg.repo().id()),
            r => r.matches(pkg.cpv()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::repo::{fake, PkgRepository};

    use super::*;

    #[test]
    fn test_ordering() {
        let mut config = Config::default();

        // unmatching pkgs sorted by dep attributes
        let r1: Repo = fake::Repo::new("b", 0).pkgs(["cat/pkg-1"]).into();
        let t = config.temp_repo("a", 0, None).unwrap();
        t.create_ebuild("cat/pkg-0", &[]).unwrap();
        let mut pkgs: Vec<_> = r1.iter().chain(t.repo.iter()).collect();
        pkgs.sort();
        let pkg_strs: Vec<_> = pkgs.iter().map(|p| p.to_string()).collect();
        assert_eq!(pkg_strs, ["cat/pkg-0::a", "cat/pkg-1::b"]);

        // matching pkgs sorted by repo priority
        let r1: Repo = fake::Repo::new("a", -1).pkgs(["cat/pkg-0"]).into();
        let t = config.temp_repo("b", 0, None).unwrap();
        t.create_ebuild("cat/pkg-0", &[]).unwrap();
        let mut pkgs: Vec<_> = r1.iter().chain(t.repo.iter()).collect();
        pkgs.sort();
        let pkg_strs: Vec<_> = pkgs.iter().map(|p| p.to_string()).collect();
        assert_eq!(pkg_strs, ["cat/pkg-0::b", "cat/pkg-0::a"]);

        // matching pkgs sorted by repo id since repos have matching priorities
        let r1: Repo = fake::Repo::new("2", 0).pkgs(["cat/pkg-0"]).into();
        let t = config.temp_repo("1", 0, None).unwrap();
        t.create_ebuild("cat/pkg-0", &[]).unwrap();
        let mut pkgs: Vec<_> = r1.iter().chain(t.repo.iter()).collect();
        pkgs.sort();
        let pkg_strs: Vec<_> = pkgs.iter().map(|p| p.to_string()).collect();
        assert_eq!(pkg_strs, ["cat/pkg-0::1", "cat/pkg-0::2"]);
    }

    #[test]
    fn package_trait_attributes() {
        let cpv = Cpv::new("cat/pkg-1-r2").unwrap();
        let r: Repo = fake::Repo::new("b", 0).pkgs([&cpv]).into();
        let pkg = r.iter_restrict(&cpv).next().unwrap();
        assert_eq!(pkg.p(), "pkg-1");
        assert_eq!(pkg.pf(), "pkg-1-r2");
        assert_eq!(pkg.pr(), "r2");
        assert_eq!(pkg.pv(), "1");
        assert_eq!(pkg.pvr(), "1-r2");
        assert_eq!(pkg.cpn(), "cat/pkg");
    }
}
