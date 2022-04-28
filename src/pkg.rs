use std::fmt;

use crate::eapi;

pub mod ebuild;
pub mod fake;

#[derive(Debug)]
pub enum Package {
    Ebuild(ebuild::Pkg),
    Fake(fake::Pkg),
}

pub trait Pkg: fmt::Debug + fmt::Display {
    fn eapi(&self) -> &eapi::Eapi;
}
