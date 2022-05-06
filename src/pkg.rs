use std::fmt;

use crate::eapi;

pub mod ebuild;
pub mod fake;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq)]
pub enum Package<'a> {
    Ebuild(ebuild::Pkg<'a>),
    Fake(fake::Pkg),
}

pub trait Pkg: fmt::Debug + fmt::Display {
    fn eapi(&self) -> &eapi::Eapi;
}
