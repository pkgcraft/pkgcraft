use std::fmt;

use crate::{atom, eapi, pkg, repo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkg<'a> {
    atom: &'a atom::Atom,
    repo: &'a repo::fake::Repo,
}

impl<'a> Pkg<'a> {
    pub(crate) fn new(atom: &'a atom::Atom, repo: &'a repo::fake::Repo) -> Self {
        Pkg { atom, repo }
    }
}

impl fmt::Display for Pkg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.atom)
    }
}

impl<'a> pkg::Package for Pkg<'a> {
    type Repo = &'a repo::fake::Repo;

    fn eapi(&self) -> &eapi::Eapi {
        &eapi::EAPI_LATEST
    }

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}
