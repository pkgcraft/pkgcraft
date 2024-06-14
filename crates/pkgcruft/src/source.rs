use pkgcraft::pkg::ebuild::{self, EbuildPackage};
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::Restrict;
use pkgcraft::types::{OrderedMap, OrderedSet};
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
    /// Restrict package version scanning to the latest version only.
    Latest,

    /// Restrict package version scanning to the latest version from each slot.
    LatestSlots,
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
        match &self.filter {
            None => Box::new(self.repo.iter_restrict(val)),
            Some(Filter::Latest) => match self.repo.iter_restrict(val).last() {
                Some(pkg) => Box::new(std::iter::once(pkg)),
                None => Box::new(std::iter::empty()),
            },
            Some(Filter::LatestSlots) => Box::new(
                self.repo
                    .iter_restrict(val)
                    .map(|pkg| (pkg.slot().to_string(), pkg))
                    .collect::<OrderedMap<_, OrderedSet<_>>>()
                    .into_iter()
                    .filter_map(|(_, mut pkgs)| pkgs.pop()),
            ),
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
        match &self.filter {
            None => Box::new(self.repo.iter_raw_restrict(val)),
            Some(Filter::Latest) => match self.repo.iter_raw_restrict(val).last() {
                Some(pkg) => Box::new(std::iter::once(pkg)),
                None => Box::new(std::iter::empty()),
            },
            Some(Filter::LatestSlots) => Box::new(
                self.repo
                    .iter_restrict(val)
                    .map(|pkg| (pkg.slot().to_string(), pkg))
                    .collect::<OrderedMap<_, OrderedSet<_>>>()
                    .into_iter()
                    .filter_map(|(_, mut pkgs)| pkgs.pop())
                    .flat_map(|pkg| self.repo.iter_raw_restrict(&pkg)),
            ),
        }
    }
}
