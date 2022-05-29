use std::fmt;

use super::{make_pkg_traits, Package};
use crate::repo::{fake::Repo, BorrowedRepo, Repository};
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
        write!(f, "{}::{}", self.atom, self.repo.id())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordering() {
        // unmatching pkgs sorted by atom
        let r1 = Repo::new("b", 0, ["cat/pkg-1"]).unwrap();
        let r2 = Repo::new("a", 0, ["cat/pkg-0"]).unwrap();
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let atoms: Vec<_> = pkgs.iter().map(|p| format!("{p}")).collect();
        assert_eq!(atoms, ["cat/pkg-0::a", "cat/pkg-1::b"]);

        // matching pkgs sorted by repo priority
        let r1 = Repo::new("a", 0, ["cat/pkg-0"]).unwrap();
        let r2 = Repo::new("b", -1, ["cat/pkg-0"]).unwrap();
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let atoms: Vec<_> = pkgs.iter().map(|p| format!("{p}")).collect();
        assert_eq!(atoms, ["cat/pkg-0::b", "cat/pkg-0::a"]);

        // matching pkgs sorted by repo id since repos have matching priorities
        let r1 = Repo::new("b", 0, ["cat/pkg-0"]).unwrap();
        let r2 = Repo::new("a", 0, ["cat/pkg-0"]).unwrap();
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let atoms: Vec<_> = pkgs.iter().map(|p| format!("{p}")).collect();
        assert_eq!(atoms, ["cat/pkg-0::a", "cat/pkg-0::b"]);
    }
}
