use std::fmt;

use crate::{eapi, repo};

pub mod ebuild;
pub mod fake;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq)]
pub enum Package<'a> {
    Ebuild(ebuild::Pkg<'a>),
    Fake(fake::Pkg<'a>),
}

pub trait Pkg: fmt::Debug + fmt::Display {
    type Repo;

    fn eapi(&self) -> &eapi::Eapi;
    fn repo(&self) -> Self::Repo;
}

impl<'a> Pkg for Package<'a> {
    type Repo = Box<&'a dyn repo::Repo>;

    fn eapi(&self) -> &eapi::Eapi {
        match self {
            Package::Ebuild(ref pkg) => pkg.eapi(),
            Package::Fake(ref pkg) => pkg.eapi(),
        }
    }

    fn repo(&self) -> Self::Repo {
        match self {
            Package::Ebuild(ref pkg) => Box::new(pkg.repo()),
            Package::Fake(ref pkg) => Box::new(pkg.repo()),
        }
    }
}

impl fmt::Display for Package<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Package::Ebuild(ref pkg) => write!(f, "{}", pkg),
            Package::Fake(ref pkg) => write!(f, "{}", pkg),
        }
    }
}
