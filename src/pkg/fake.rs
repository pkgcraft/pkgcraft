use std::fmt;

use crate::{atom, eapi, pkg, repo, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkg<'a> {
    atom: &'a atom::Atom,
    repo: &'a repo::fake::Repo,
}

impl<'a> Pkg<'a> {
    pub fn new(atom: &'a atom::Atom, repo: &'a repo::fake::Repo) -> Result<Self> {
        Ok(Pkg { atom, repo })
    }
}

impl fmt::Display for Pkg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.atom)
    }
}

impl pkg::Pkg for Pkg<'_> {
    type Repo = repo::fake::Repo;

    fn eapi(&self) -> &eapi::Eapi {
        &eapi::EAPI_LATEST
    }

    fn repo(&self) -> &Self::Repo {
        self.repo
    }
}
