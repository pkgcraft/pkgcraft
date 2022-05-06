use std::fmt;

use crate::eapi;

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
    fn repo(&self) -> &Self::Repo;
}
