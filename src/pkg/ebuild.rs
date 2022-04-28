use std::fmt;
use std::path::{Path, PathBuf};

use crate::{atom, eapi, pkg};

#[derive(Debug)]
pub struct Pkg {
    atom: atom::Atom,
    path: PathBuf,
}

impl Pkg {
    pub fn slot(&self) -> Option<&str> {
        self.atom.slot()
    }

    pub fn subslot(&self) -> Option<&str> {
        self.atom.slot()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for Pkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.path())
    }
}

impl pkg::Pkg for Pkg {
    fn eapi(&self) -> &eapi::Eapi {
        unimplemented!();
    }
}
