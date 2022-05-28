use std::fmt;

use crate::repo::{fake::Repo, BorrowedRepo};
use crate::{atom, eapi, pkg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkg<'a> {
    atom: &'a atom::Atom,
    repo: &'a Repo,
}

impl<'a> Pkg<'a> {
    pub(crate) fn new(atom: &'a atom::Atom, repo: &'a Repo) -> Self {
        Pkg { atom, repo }
    }
}

impl fmt::Display for Pkg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.atom)
    }
}

impl<'a> pkg::Package for Pkg<'a> {
    type Repo = BorrowedRepo<'a>;

    fn atom(&self) -> &atom::Atom {
        self.atom
    }

    fn eapi(&self) -> &eapi::Eapi {
        &eapi::EAPI_LATEST
    }

    fn repo(&self) -> Self::Repo {
        BorrowedRepo::Fake(self.repo)
    }
}
