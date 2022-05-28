use std::fmt;

use super::{make_pkg_traits, Package};
use crate::repo::{fake::Repo, BorrowedRepo};
use crate::{atom, eapi};

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    atom: &'a atom::Atom,
    repo: &'a Repo,
}

make_pkg_traits!(Pkg<'_>);

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

impl<'a> Package for Pkg<'a> {
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
