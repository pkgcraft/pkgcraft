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

/// Package filtering variants.
#[derive(
    AsRefStr, Display, EnumIter, EnumString, VariantNames, Debug, PartialEq, Eq, Hash, Copy, Clone,
)]
#[strum(serialize_all = "kebab-case")]
pub enum Filter {
    Latest,
}

pub(crate) trait IterRestrict {
    type Item;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Box<dyn Iterator<Item = Self::Item>>;
}

pub(crate) struct Ebuild {
    pub(crate) repo: &'static Repo,
    pub(crate) filter: Option<Filter>,
}

impl IterRestrict for Ebuild {
    type Item = ebuild::Pkg<'static>;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Box<dyn Iterator<Item = Self::Item>> {
        if self.filter.is_some() {
            match self.repo.iter_restrict(val).last() {
                Some(value) => Box::new(std::iter::once(value)),
                None => Box::new(std::iter::empty()),
            }
        } else {
            Box::new(self.repo.iter_restrict(val))
        }
    }
}

pub(crate) struct EbuildRaw {
    pub(crate) repo: &'static Repo,
    pub(crate) filter: Option<Filter>,
}

impl IterRestrict for EbuildRaw {
    type Item = ebuild::raw::Pkg<'static>;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Box<dyn Iterator<Item = Self::Item>> {
        if self.filter.is_some() {
            match self.repo.iter_raw_restrict(val).last() {
                Some(value) => Box::new(std::iter::once(value)),
                None => Box::new(std::iter::empty()),
            }
        } else {
            Box::new(self.repo.iter_raw_restrict(val))
        }
    }
}
