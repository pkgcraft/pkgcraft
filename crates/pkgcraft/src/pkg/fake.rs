use crate::atom::Atom;
use crate::eapi::{Eapi, EAPI_LATEST};
use crate::pkg;
use crate::repo::{fake::Repo, Repository};
use crate::restrict::atom::Restrict as AtomRestrict;
use crate::restrict::{Restrict as BaseRestrict, Restriction};

use super::{make_pkg_traits, Package};

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    cpv: Atom,
    repo: &'a Repo,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(cpv: &'a Atom, repo: &'a Repo) -> Self {
        Self { cpv: cpv.clone(), repo }
    }
}

impl<'a> Package for Pkg<'a> {
    type Repo = &'a Repo;

    fn cpv(&self) -> &Atom {
        &self.cpv
    }

    fn eapi(&self) -> &'static Eapi {
        &EAPI_LATEST
    }

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

impl Restriction<&Pkg<'_>> for BaseRestrict {
    fn matches(&self, pkg: &Pkg) -> bool {
        use BaseRestrict::*;
        crate::restrict::restrict_match! {self, pkg,
            Atom(r) => r.matches(pkg),
            Pkg(r) => r.matches(pkg),
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for AtomRestrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use AtomRestrict::*;
        match self {
            Repo(Some(r)) => r.matches(pkg.repo().id()),
            r => r.matches(pkg.cpv()),
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for pkg::Restrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use pkg::Restrict::*;
        match self {
            Eapi(r) => r.matches(pkg.eapi().as_str()),
            Repo(r) => r.matches(pkg.repo().id()),
            _ => false,
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
