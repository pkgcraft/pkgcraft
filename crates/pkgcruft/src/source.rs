use pkgcraft::pkg::ebuild;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Restrict;
use strum::{AsRefStr, EnumIter, EnumString};

use crate::runner::*;

/// All check runner source variants.
#[derive(
    AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub(crate) enum SourceKind {
    Ebuild,
    EbuildRaw,
}

impl SourceKind {
    /// Create a new check runner for a source variant.
    pub(crate) fn new_runner<'a>(&self, repo: &'a Repo) -> CheckRunner<'a> {
        match self {
            Self::Ebuild => CheckRunner::EbuildPkg(EbuildPkgCheckRunner::new(repo)),
            Self::EbuildRaw => CheckRunner::EbuildRawPkg(EbuildRawPkgCheckRunner::new(repo)),
        }
    }
}

// TODO: return impl Iterator once MSRV >= 1.75
pub(crate) trait IterRestrict {
    type Item;
    fn iter_restrict<R: Into<Restrict>>(&self, val: R)
        -> Box<dyn Iterator<Item = Self::Item> + '_>;
}

#[derive(Debug)]
pub(crate) struct Ebuild<'a> {
    pub(crate) repo: &'a Repo,
}

impl<'a> IterRestrict for Ebuild<'a> {
    type Item = ebuild::Pkg<'a>;

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> Box<dyn Iterator<Item = Self::Item> + '_> {
        Box::new(self.repo.iter_restrict(val))
    }
}

#[derive(Debug)]
pub(crate) struct EbuildRaw<'a> {
    pub(crate) repo: &'a Repo,
}

impl<'a> IterRestrict for EbuildRaw<'a> {
    type Item = ebuild::raw::Pkg<'a>;

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> Box<dyn Iterator<Item = Self::Item> + '_> {
        Box::new(self.repo.iter_raw_restrict(val))
    }
}
