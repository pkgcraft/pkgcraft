use std::str::FromStr;

use pkgcraft::pkg::ebuild::{self, EbuildPackage};
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::{self, Restrict};
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{Display, EnumIter, EnumString, VariantNames};

use crate::Error;

/// All check runner source variants.
#[derive(
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
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Filter {
    /// Restrict package version scanning to the latest version only.
    Latest,

    /// Restrict package version scanning to the latest version from each slot.
    LatestSlots,

    /// Restrict package version scanning with a custom restriction.
    Restrict(Restrict),
}

impl FromStr for Filter {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match s {
            "latest" => Ok(Self::Latest),
            "latest-slots" => Ok(Self::LatestSlots),
            _ => restrict::parse::pkg(s)
                .map(Self::Restrict)
                .map_err(|e| Error::InvalidValue(format!("{e}"))),
        }
    }
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
            Some(Filter::Restrict(restrict)) => Box::new(
                self.repo
                    .iter_restrict(Restrict::and([val.into(), restrict.clone()])),
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
            Some(Filter::Restrict(restrict)) => Box::new(
                self.repo
                    .iter_restrict(Restrict::and([val.into(), restrict.clone()]))
                    .flat_map(|pkg| self.repo.iter_raw_restrict(&pkg)),
            ),
        }
    }
}
