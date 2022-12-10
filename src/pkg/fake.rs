use crate::repo::{fake::Repo, Repository};
use crate::restrict::{self, Restriction};
use crate::{atom, eapi, pkg};

use super::{make_pkg_traits, Package};

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    atom: atom::Atom,
    repo: &'a Repo,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(atom: &'a atom::Atom, repo: &'a Repo) -> Self {
        Pkg {
            atom: atom.clone(),
            repo,
        }
    }
}

impl<'a> Package for Pkg<'a> {
    type Repo = &'a Repo;

    fn atom(&self) -> &atom::Atom {
        &self.atom
    }

    fn eapi(&self) -> &'static eapi::Eapi {
        &eapi::EAPI_LATEST
    }

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

impl Restriction<&Pkg<'_>> for restrict::Restrict {
    fn matches(&self, pkg: &Pkg) -> bool {
        restrict::restrict_match! {self, pkg,
            Self::Atom(atom::Restrict::Repo(Some(r))) => r.matches(pkg.repo().id()),
            Self::Atom(r) => r.matches(pkg.atom()),
            Self::Pkg(pkg::Restrict::Eapi(r)) => r.matches(pkg.eapi().as_str()),
            Self::Pkg(pkg::Restrict::Repo(r)) => r.matches(pkg.repo().id()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::repo::PkgRepository;

    use super::*;

    #[test]
    fn test_ordering() {
        // unmatching pkgs sorted by atom
        let r1 = Repo::new("b", 0, ["cat/pkg-1"]);
        let r2 = Repo::new("a", 0, ["cat/pkg-0"]);
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let atoms: Vec<_> = pkgs.iter().map(|p| p.to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-0::a", "cat/pkg-1::b"]);

        // matching pkgs sorted by repo priority
        let r1 = Repo::new("a", -1, ["cat/pkg-0"]);
        let r2 = Repo::new("b", 0, ["cat/pkg-0"]);
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let atoms: Vec<_> = pkgs.iter().map(|p| p.to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-0::b", "cat/pkg-0::a"]);

        // matching pkgs sorted by repo id since repos have matching priorities
        let r1 = Repo::new("b", 0, ["cat/pkg-0"]);
        let r2 = Repo::new("a", 0, ["cat/pkg-0"]);
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let atoms: Vec<_> = pkgs.iter().map(|p| p.to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-0::a", "cat/pkg-0::b"]);
    }
}
