use pkgcraft::pkg::ebuild;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Restrict;
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

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
