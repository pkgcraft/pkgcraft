use pkgcraft::pkg::ebuild;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Restrict;
use strum::{AsRefStr, EnumIter, EnumString};

#[derive(
    AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum SourceKind {
    EbuildPackage,
    EbuildPackageRaw,
    EbuildPackageSet,
}

// TODO: return impl Iterator once MSRV >= 1.75
pub(crate) trait IterRestrict {
    type Item;
    fn iter_restrict<R: Into<Restrict>>(&self, val: R)
        -> Box<dyn Iterator<Item = Self::Item> + '_>;
}

#[derive(Debug, Clone)]
pub(crate) struct EbuildPackage<'a> {
    pub(crate) repo: &'a Repo,
}

impl<'a> IterRestrict for EbuildPackage<'a> {
    type Item = ebuild::Pkg<'a>;

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> Box<dyn Iterator<Item = Self::Item> + '_> {
        Box::new(self.repo.iter_restrict(val))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EbuildPackageRaw<'a> {
    pub(crate) repo: &'a Repo,
}

impl<'a> IterRestrict for EbuildPackageRaw<'a> {
    type Item = ebuild::raw::Pkg<'a>;

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> Box<dyn Iterator<Item = Self::Item> + '_> {
        Box::new(self.repo.iter_raw_restrict(val))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EbuildPackageSet<'a> {
    pub(crate) repo: &'a Repo,
}

impl<'a> IterRestrict for EbuildPackageSet<'a> {
    type Item = Vec<ebuild::Pkg<'a>>;

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> Box<dyn Iterator<Item = Self::Item> + '_> {
        let pkgs: Vec<_> = self.repo.iter_restrict(val).collect();
        if pkgs.is_empty() {
            Box::new(std::iter::empty())
        } else {
            Box::new(std::iter::once(pkgs))
        }
    }
}
