use std::str::FromStr;

use colored::{Color, Colorize};
use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::KeywordStatus;
use pkgcraft::pkg::ebuild::{self, EbuildPackage};
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::{self, Restrict};
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};

use crate::Error;

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
#[derive(AsRefStr, EnumIter, Debug, PartialEq, Eq, Hash, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum Filter {
    /// Restrict package version scanning to the latest version only.
    Latest,

    /// Restrict package version scanning to the latest version from each slot.
    LatestSlots,

    /// Restrict package version scanning with a custom restriction.
    Restrict(Restrict),

    /// Restrict package version scanning to packages with only stable keywords.
    Stable,

    /// Restrict package version scanning to packages with only unstable keywords.
    Unstable,
}

impl FromStr for Filter {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match s.trim() {
            "latest" => Ok(Self::Latest),
            "latest-slots" => Ok(Self::LatestSlots),
            "stable" => Ok(Self::Stable),
            "unstable" => Ok(Self::Unstable),
            s if s.contains(|c: char| c.is_whitespace()) => restrict::parse::pkg(s)
                .map(Self::Restrict)
                .map_err(|e| Error::InvalidValue(format!("{e}"))),
            s => {
                let possible = Filter::iter()
                    .filter(|r| !matches!(r, Filter::Restrict(_)))
                    .map(|r| r.as_ref().color(Color::Green))
                    .join(", ");
                let message = indoc::formatdoc! {r#"
                    invalid filter: {s}
                      [possible values: {possible}]

                    Custom restrictions are also supported. For example, to target all packages
                    maintained by the python project use the following command:

                    pkgcruft scan -f "maintainers any email == 'python@gentoo.org'""#};
                Err(Error::InvalidValue(message))
            }
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
            Some(Filter::Stable) => Box::new(self.repo.iter_restrict(val).filter(|pkg| {
                !pkg.keywords().is_empty()
                    && pkg
                        .keywords()
                        .iter()
                        .all(|k| k.status() == KeywordStatus::Stable)
            })),
            Some(Filter::Unstable) => Box::new(self.repo.iter_restrict(val).filter(|pkg| {
                !pkg.keywords().is_empty()
                    && pkg
                        .keywords()
                        .iter()
                        .all(|k| k.status() == KeywordStatus::Unstable)
            })),
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
            Some(Filter::Stable) => Box::new(
                self.repo
                    .iter_restrict(val)
                    .filter(|pkg| {
                        !pkg.keywords().is_empty()
                            && pkg
                                .keywords()
                                .iter()
                                .all(|k| k.status() == KeywordStatus::Stable)
                    })
                    .flat_map(|pkg| self.repo.iter_raw_restrict(&pkg)),
            ),
            Some(Filter::Unstable) => Box::new(
                self.repo
                    .iter_restrict(val)
                    .filter(|pkg| {
                        !pkg.keywords().is_empty()
                            && pkg
                                .keywords()
                                .iter()
                                .all(|k| k.status() == KeywordStatus::Unstable)
                    })
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
