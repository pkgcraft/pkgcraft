use crate::dep::Cpv;
use crate::eapi::{Eapi, EAPI_LATEST_OFFICIAL};
use crate::pkg;
use crate::repo::{fake::Repo, Repository};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::{Restrict as BaseRestrict, Restriction};

use super::{make_pkg_traits, Package, RepoPackage};

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    cpv: Cpv<String>,
    repo: &'a Repo,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(cpv: &'a Cpv<String>, repo: &'a Repo) -> Self {
        Self { cpv: cpv.clone(), repo }
    }
}

impl<'a> Package for Pkg<'a> {
    fn eapi(&self) -> &'static Eapi {
        &EAPI_LATEST_OFFICIAL
    }

    fn cpv(&self) -> &Cpv<String> {
        &self.cpv
    }
}

impl<'a> RepoPackage for Pkg<'a> {
    type Repo = &'a Repo;

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

impl Restriction<&Pkg<'_>> for BaseRestrict {
    fn matches(&self, pkg: &Pkg) -> bool {
        use BaseRestrict::*;
        crate::restrict::restrict_match! {self, pkg,
            Dep(r) => r.matches(pkg),
            Pkg(r) => r.matches(pkg),
        }
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for DepRestrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        use DepRestrict::*;
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
            Eapi(r) => r.matches(pkg.eapi()),
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
    fn ordering() {
        // unmatching pkgs sorted by dep attributes
        let r1 = Repo::new("b", 0).pkgs(["cat/pkg-1"]);
        let r2 = Repo::new("a", 0).pkgs(["cat/pkg-0"]);
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let pkg_strs: Vec<_> = pkgs.iter().map(|p| p.to_string()).collect();
        assert_eq!(pkg_strs, ["cat/pkg-0::a", "cat/pkg-1::b"]);

        // matching pkgs sorted by repo priority
        let r1 = Repo::new("a", -1).pkgs(["cat/pkg-0"]);
        let r2 = Repo::new("b", 0).pkgs(["cat/pkg-0"]);
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let pkg_strs: Vec<_> = pkgs.iter().map(|p| p.to_string()).collect();
        assert_eq!(pkg_strs, ["cat/pkg-0::b", "cat/pkg-0::a"]);

        // matching pkgs sorted by repo id since repos have matching priorities
        let r1 = Repo::new("b", 0).pkgs(["cat/pkg-0"]);
        let r2 = Repo::new("a", 0).pkgs(["cat/pkg-0"]);
        let mut pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        pkgs.sort();
        let pkg_strs: Vec<_> = pkgs.iter().map(|p| p.to_string()).collect();
        assert_eq!(pkg_strs, ["cat/pkg-0::a", "cat/pkg-0::b"]);
    }
}
