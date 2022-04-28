use std::fmt;

use crate::{atom, eapi, pkg};

#[derive(Debug)]
pub struct Pkg {
    atom: atom::Atom,
}

impl fmt::Display for Pkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.atom)
    }
}

impl pkg::Pkg for Pkg {
    fn eapi(&self) -> &eapi::Eapi {
        eapi::EAPI_LATEST
    }
}
