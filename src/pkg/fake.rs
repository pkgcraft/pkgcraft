use std::fmt;

use crate::{atom, eapi, pkg, Result};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Pkg {
    atom: atom::Atom,
}

impl Pkg {
    pub fn new<S: AsRef<str>>(cpv: S) -> Result<Self> {
        let atom = atom::parse::cpv(cpv.as_ref())?;
        Ok(Pkg { atom })
    }
}

impl fmt::Display for Pkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.atom)
    }
}

impl pkg::Pkg for Pkg {
    fn eapi(&self) -> &eapi::Eapi {
        &eapi::EAPI_LATEST
    }
}
