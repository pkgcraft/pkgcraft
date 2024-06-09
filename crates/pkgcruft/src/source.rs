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

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item>;
}

pub(crate) struct Ebuild {
    pub(crate) repo: &'static Repo,
}

impl IterRestrict for Ebuild {
    type Item = ebuild::Pkg<'static>;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item> {
        self.repo.iter_restrict(val)
    }
}

pub(crate) struct EbuildRaw {
    pub(crate) repo: &'static Repo,
}

impl IterRestrict for EbuildRaw {
    type Item = ebuild::raw::Pkg<'static>;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> impl Iterator<Item = Self::Item> {
        self.repo.iter_raw_restrict(val)
    }
}
