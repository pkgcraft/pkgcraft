use crate::dep::{Cpv, Dep};
use crate::eapi::{Eapi, EAPI_LATEST_OFFICIAL};
use crate::macros::bool_not_equal;
use crate::pkg;
use crate::repo::{fake::Repo, Repository};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::Intersects;

use super::{make_pkg_traits, Package, RepoPackage};

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    cpv: Cpv,
    repo: &'a Repo,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(cpv: &'a Cpv, repo: &'a Repo) -> Self {
        Self { cpv: cpv.clone(), repo }
    }
}

impl<'a> Package for Pkg<'a> {
    fn eapi(&self) -> &'static Eapi {
        &EAPI_LATEST_OFFICIAL
    }

    fn cpv(&self) -> &Cpv {
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

impl Intersects<Dep> for Pkg<'_> {
    fn intersects(&self, dep: &Dep) -> bool {
        bool_not_equal!(self.cpn(), dep.cpn());

        if dep.slot().is_some() {
            return false;
        }

        if dep.subslot().is_some() {
            return false;
        }

        if dep.use_deps().is_some() {
            return false;
        }

        if let Some(val) = dep.repo() {
            bool_not_equal!(self.repo.name(), val);
        }

        if let Some(val) = dep.version() {
            self.cpv().version().intersects(val)
        } else {
            true
        }
    }
}

impl Intersects<Pkg<'_>> for Dep {
    fn intersects(&self, other: &Pkg<'_>) -> bool {
        other.intersects(self)
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::repo::PkgRepository;
    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn ordering() {
        // unmatching pkgs sorted by dep attributes
        let r1 = Repo::new("b", 0).pkgs(["cat/pkg-1"]);
        let r2 = Repo::new("a", 0).pkgs(["cat/pkg-0"]);
        let pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        let sorted_pkgs: Vec<_> = pkgs.clone().into_iter().sorted().collect();
        assert_ordered_eq!(pkgs.iter().rev(), &sorted_pkgs);

        // matching pkgs sorted by repo priority
        let r1 = Repo::new("a", -1).pkgs(["cat/pkg-0"]);
        let r2 = Repo::new("b", 0).pkgs(["cat/pkg-0"]);
        let pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        let sorted_pkgs: Vec<_> = pkgs.clone().into_iter().sorted().collect();
        assert_ordered_eq!(pkgs.iter().rev(), &sorted_pkgs);

        // matching pkgs sorted by repo id since repos have matching priorities
        let r1 = Repo::new("b", 0).pkgs(["cat/pkg-0"]);
        let r2 = Repo::new("a", 0).pkgs(["cat/pkg-0"]);
        let pkgs: Vec<_> = r1.iter().chain(r2.iter()).collect();
        let sorted_pkgs: Vec<_> = pkgs.clone().into_iter().sorted().collect();
        assert_ordered_eq!(pkgs.iter().rev(), &sorted_pkgs);
    }
}
