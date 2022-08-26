use std::fmt;

use enum_as_inner::EnumAsInner;
use scallop::variables::bind;
use strum::{AsRefStr, EnumIter, IntoEnumIterator};

use crate::repo::{Repo, Repository};
use crate::restrict::{self, Restriction};
use crate::{atom, eapi};

pub mod ebuild;
pub mod fake;

#[derive(AsRefStr, EnumIter, Debug, Copy, Clone)]
pub enum Env {
    P,
    PN,
    PV,
    PR,
    PVR,
    PF,
    CATEGORY,
}

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

    /// Return a package's atom.
    fn atom(&self) -> &atom::Atom;

    /// Return a package's version.
    fn version(&self) -> &atom::Version {
        self.atom().version().unwrap()
    }

    /// Return a package's value for a specified environment variable.
    fn env(&self, var: Env) -> String {
        let (a, v) = (self.atom(), self.version());
        use Env::*;
        match var {
            P => format!("{}-{}", a.package(), v.base()),
            PN => a.package().into(),
            PV => v.base().into(),
            PR => format!("r{}", v.revision()),
            PVR => match v.revision() == "0" {
                true => v.base().into(),
                false => v.into(),
            },
            PF => format!("{}-{}", a.package(), self.env(PVR)),
            CATEGORY => a.category().into(),
        }
    }
}

pub(crate) trait PackageEnv: Package {
    fn export_env(&self) -> crate::Result<()> {
        for var in Env::iter() {
            bind(var, self.env(var), None, None)?;
        }
        Ok(())
    }
}

macro_rules! make_pkg_traits {
    ($($x:ty),+) => {$(
        impl PartialEq for $x {
            fn eq(&self, other: &Self) -> bool {
                self.repo() == other.repo() && self.atom() == other.atom()
            }
        }

        impl Eq for $x {}

        impl std::hash::Hash for $x {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.repo().hash(state);
                self.atom().hash(state);
            }
        }

        impl PartialOrd for $x {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $x {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                crate::macros::cmp_not_equal!(self.atom(), other.atom());
                self.repo().cmp(&other.repo())
            }
        }

        impl fmt::Display for $x {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                use crate::repo::Repository;
                write!(f, "{}::{}", self.atom(), self.repo().id())
            }
        }

        impl From<&$x> for crate::restrict::Restrict {
            fn from(pkg: &Pkg) -> Self {
                Self::from(pkg.atom())
            }
        }

        impl crate::pkg::PackageEnv for $x {}
    )+};
}
pub(self) use make_pkg_traits;

impl<'a> Package for Pkg<'a> {
    type Repo = &'a Repo;

    fn atom(&self) -> &atom::Atom {
        match self {
            Self::Ebuild(pkg, _) => pkg.atom(),
            Self::Fake(pkg, _) => pkg.atom(),
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

#[derive(Debug, Clone)]
pub enum Restrict {
    Eapi(restrict::Str),
    Ebuild(ebuild::Restrict),
    Repo(restrict::Str),
}

impl From<Restrict> for restrict::Restrict {
    fn from(r: Restrict) -> Self {
        Self::Pkg(r)
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for Restrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        match self {
            Self::Eapi(r) => r.matches(pkg.eapi().as_str()),
            Self::Repo(r) => r.matches(pkg.repo().id()),
            Self::Ebuild(r) => match pkg {
                Pkg::Ebuild(p, _) => r.matches(p),
                _ => false,
            },
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for restrict::Restrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        restrict::restrict_match! {
            self, pkg,
            Self::Atom(r) => r.matches(pkg.atom()),
            Self::Pkg(r) => r.matches(pkg)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::repo::{fake, Repo};

    #[test]
    fn test_ordering() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();

        // unmatching pkgs sorted by atom
        let r1: Repo = fake::Repo::new("b", 0, ["cat/pkg-1"]).unwrap().into();
        let (t, repo) = config.temp_repo("a", 0).unwrap();
        t.create_ebuild("cat/pkg-0", []).unwrap();
        let r2 = Repo::Ebuild(repo);
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let atoms: Vec<_> = pkgs.iter().map(|p| format!("{p}")).collect();
        assert_eq!(atoms, ["cat/pkg-0::a", "cat/pkg-1::b"]);

        // matching pkgs sorted by repo priority
        let r1: Repo = fake::Repo::new("a", 0, ["cat/pkg-0"]).unwrap().into();
        let (t, repo) = config.temp_repo("b", -1).unwrap();
        t.create_ebuild("cat/pkg-0", []).unwrap();
        let r2 = Repo::Ebuild(repo);
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let atoms: Vec<_> = pkgs.iter().map(|p| format!("{p}")).collect();
        assert_eq!(atoms, ["cat/pkg-0::b", "cat/pkg-0::a"]);

        // matching pkgs sorted by repo id since repos have matching priorities
        let r1: Repo = fake::Repo::new("2", 0, ["cat/pkg-0"]).unwrap().into();
        let (t, repo) = config.temp_repo("1", 0).unwrap();
        t.create_ebuild("cat/pkg-0", []).unwrap();
        let r2 = Repo::Ebuild(repo);
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let atoms: Vec<_> = pkgs.iter().map(|p| format!("{p}")).collect();
        assert_eq!(atoms, ["cat/pkg-0::1", "cat/pkg-0::2"]);
    }
}
