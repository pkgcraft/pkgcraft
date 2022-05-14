use std::fmt;

use crate::eapi;
use crate::repo::Repository;

pub mod ebuild;
pub mod fake;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq)]
pub enum Pkg<'a> {
    Ebuild(ebuild::Pkg<'a>),
    Fake(fake::Pkg<'a>),
}

pub trait Package: fmt::Debug + fmt::Display {
    type Repo;

    fn eapi(&self) -> &eapi::Eapi;
    fn repo(&self) -> Self::Repo;
}

impl<'a> Package for Pkg<'a> {
    type Repo = Box<&'a dyn Repository>;

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
