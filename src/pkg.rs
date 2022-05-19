use std::fmt;

use crate::repo::Repository;
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
#[derive(Debug, PartialEq)]
pub enum Pkg<'a> {
    Ebuild(ebuild::Pkg<'a>),
    Fake(fake::Pkg<'a>),
}

pub trait Package: fmt::Debug + fmt::Display {
    type Repo;

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
        match var {
            Env::P => format!("{}-{}", a.package(), v.base()),
            Env::PN => a.package().into(),
            Env::PV => v.base().into(),
            Env::PR => format!("r{}", v.revision().as_str()),
            Env::PVR => match v.revision() == "0" {
                true => v.base().into(),
                false => v.into(),
            },
            Env::PF => format!("{}-{}", a.package(), self.env(Env::PVR)),
            Env::CATEGORY => a.category().into(),
        }
    }
}

impl<'a> Package for Pkg<'a> {
    type Repo = Box<&'a dyn Repository>;

    fn atom(&self) -> &atom::Atom {
        match self {
            Pkg::Ebuild(ref pkg) => pkg.atom(),
            Pkg::Fake(ref pkg) => pkg.atom(),
        }
    }

    fn eapi(&self) -> &eapi::Eapi {
        match self {
            Pkg::Ebuild(ref pkg) => pkg.eapi(),
            Pkg::Fake(ref pkg) => pkg.eapi(),
        }
    }

    fn repo(&self) -> Self::Repo {
        match self {
            Pkg::Ebuild(ref pkg) => Box::new(pkg.repo()),
            Pkg::Fake(ref pkg) => Box::new(pkg.repo()),
        }
    }
}

impl fmt::Display for Pkg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Pkg::Ebuild(ref pkg) => write!(f, "{}", pkg),
            Pkg::Fake(ref pkg) => write!(f, "{}", pkg),
        }
    }
}
