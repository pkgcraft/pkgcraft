use crate::dep::Cpv;
use crate::eapi::Eapi;
use crate::pkg::{make_pkg_traits, Package};
use crate::repo::ebuild::configured::Repo;
use crate::restrict::{Restrict as BaseRestrict, Restriction};

#[derive(Debug)]
pub struct Pkg<'a> {
    repo: &'a Repo,
    raw: super::Pkg<'a>,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(repo: &'a Repo, raw: super::Pkg<'a>) -> Self {
        Self { repo, raw }
    }
}

impl<'a> Package for Pkg<'a> {
    type Repo = &'a Repo;

    fn cpv(&self) -> &Cpv {
        self.raw.cpv()
    }

    fn eapi(&self) -> &'static Eapi {
        self.raw.eapi()
    }

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

impl<'a> Restriction<&'a Pkg<'a>> for BaseRestrict {
    fn matches(&self, pkg: &'a Pkg<'a>) -> bool {
        self.matches(&pkg.raw)
    }
}
