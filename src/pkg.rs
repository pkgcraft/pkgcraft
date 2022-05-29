use std::fmt;

use crate::repo::{BorrowedRepo, Repository};
use crate::{atom, eapi};

pub mod ebuild;
pub mod fake;

#[derive(Debug)]
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
#[derive(Debug)]
pub enum Pkg<'a> {
    Ebuild(ebuild::Pkg<'a>),
    Fake(fake::Pkg<'a>),
}

make_pkg_traits!(Pkg<'_>);

pub trait Package: fmt::Debug + fmt::Display + PartialEq + Eq + PartialOrd + Ord {
    type Repo: Repository;

    /// Get a package's EAPI.
    fn eapi(&self) -> &eapi::Eapi;

    /// Get a package's repo.
    fn repo(&self) -> Self::Repo;

    /// Get a package's atom.
    fn atom(&self) -> &atom::Atom;

    /// Get a package's version.
    fn version(&self) -> &atom::Version {
        self.atom().version().unwrap()
    }

    /// Get a package's value for a specified environment variable.
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

macro_rules! make_pkg_traits {
    ($($x:ty),*) => {
        $(
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
        )*
    };
}
pub(self) use make_pkg_traits;

impl<'a> Package for Pkg<'a> {
    type Repo = BorrowedRepo<'a>;

    fn atom(&self) -> &atom::Atom {
        match self {
            Self::Ebuild(ref pkg) => pkg.atom(),
            Self::Fake(ref pkg) => pkg.atom(),
        }
    }

    fn eapi(&self) -> &eapi::Eapi {
        match self {
            Self::Ebuild(ref pkg) => pkg.eapi(),
            Self::Fake(ref pkg) => pkg.eapi(),
        }
    }

    fn repo(&self) -> Self::Repo {
        match self {
            Self::Ebuild(ref pkg) => pkg.repo(),
            Self::Fake(ref pkg) => pkg.repo(),
        }
    }
}
