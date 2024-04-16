use pkgcraft::pkg::ebuild;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Restrict;
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

use crate::runner::*;

/// All check runner source variants.
#[derive(
    AsRefStr,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
)]
#[strum(serialize_all = "kebab-case")]
pub enum SourceKind {
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

pub(crate) trait IterRestrict {
    type Item;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item> + '_;
}

#[derive(Debug)]
pub(crate) struct Ebuild<'a> {
    pub(crate) repo: &'a Repo,
}

impl<'a> IterRestrict for Ebuild<'a> {
    type Item = ebuild::Pkg<'a>;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item> + '_ {
        self.repo.iter_restrict(val)
    }
}

#[derive(Debug)]
pub(crate) struct EbuildRaw<'a> {
    pub(crate) repo: &'a Repo,
}

impl<'a> IterRestrict for EbuildRaw<'a> {
    type Item = ebuild::raw::Pkg<'a>;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item> + '_ {
        self.repo.iter_raw_restrict(val)
    }
}
