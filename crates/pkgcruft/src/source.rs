use std::str::FromStr;

use colored::{Color, Colorize};
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::pkg::ebuild::keyword::KeywordStatus;
use pkgcraft::pkg::ebuild::{self, EbuildPackage};
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::{self, Restrict, Restriction};
use pkgcraft::traits::Contains;
use pkgcraft::types::OrderedMap;
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
pub enum PkgFilter {
    /// Filter packages using the latest version only.
    Latest,

    /// Filter packages using the latest version from each slot.
    LatestSlots,

    /// Filter packages based on live status.
    Live(bool),

    /// Filter packages based on global mask status.
    Masked(bool),

    /// Filter packages using a custom restriction.
    Restrict(bool, Restrict),

    /// Filter packages based on stable keyword status.
    Stable(bool),
}

impl FromStr for PkgFilter {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let stripped = s.strip_prefix('!');
        let inverted = stripped.is_some();
        match stripped.unwrap_or(s) {
            "latest" | "latest-slots" if inverted => {
                Err(Error::InvalidValue("filter doesn't support inversion".to_string()))
            }
            "latest" => Ok(PkgFilter::Latest),
            "latest-slots" => Ok(PkgFilter::LatestSlots),
            "live" => Ok(PkgFilter::Live(inverted)),
            "masked" => Ok(PkgFilter::Masked(inverted)),
            "stable" => Ok(PkgFilter::Stable(inverted)),
            s if s.contains(|c: char| c.is_whitespace()) => restrict::parse::pkg(s)
                .map(|r| PkgFilter::Restrict(inverted, r))
                .map_err(|e| Error::InvalidValue(format!("{e}"))),
            s => {
                let possible = PkgFilter::iter()
                    .filter(|r| !matches!(r, PkgFilter::Restrict(_, _)))
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

/// Layered package filtering support.
#[derive(Debug, PartialEq, Eq, Clone)]
struct PkgFilters(IndexSet<PkgFilter>);

impl PkgFilters {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        repo: &'static Repo,
        val: R,
    ) -> Box<dyn Iterator<Item = ebuild::Pkg<'static>> + '_> {
        let mut iter: Box<dyn Iterator<Item = ebuild::Pkg<'static>>> =
            Box::new(repo.iter_restrict(val));

        for filter in &self.0 {
            iter = match filter {
                PkgFilter::Latest => match iter.last() {
                    Some(pkg) => Box::new(std::iter::once(pkg)),
                    None => Box::new(std::iter::empty()),
                },
                PkgFilter::LatestSlots => Box::new(
                    iter.map(|pkg| (pkg.slot().to_string(), pkg))
                        .collect::<OrderedMap<_, Vec<_>>>()
                        .into_values()
                        .filter_map(|mut pkgs| pkgs.pop()),
                ),
                PkgFilter::Live(inverted) => {
                    Box::new(iter.filter(move |pkg| inverted ^ pkg.properties().contains("live")))
                }
                PkgFilter::Masked(inverted) => {
                    Box::new(iter.filter(move |pkg| inverted ^ pkg.masked()))
                }
                PkgFilter::Stable(inverted) => {
                    let status = if *inverted {
                        KeywordStatus::Unstable
                    } else {
                        KeywordStatus::Stable
                    };
                    Box::new(iter.filter(move |pkg| {
                        !pkg.keywords().is_empty()
                            && pkg.keywords().iter().all(|k| k.status() == status)
                    }))
                }
                PkgFilter::Restrict(inverted, restrict) => {
                    Box::new(iter.filter(move |pkg| inverted ^ restrict.matches(pkg)))
                }
            }
        }

        iter
    }
}

pub(crate) trait IterRestrict {
    type Item;

    fn iter_restrict<R: Into<Restrict>>(&self, val: R)
        -> Box<dyn Iterator<Item = Self::Item> + '_>;
}

pub(crate) struct Ebuild {
    repo: &'static Repo,
    filters: PkgFilters,
}

impl Ebuild {
    pub(crate) fn new(repo: &'static Repo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            repo,
            filters: PkgFilters(filters),
        }
    }
}

impl IterRestrict for Ebuild {
    type Item = ebuild::Pkg<'static>;

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> Box<dyn Iterator<Item = Self::Item> + '_> {
        if self.filters.is_empty() {
            Box::new(self.repo.iter_restrict(val))
        } else {
            Box::new(self.filters.iter_restrict(self.repo, val))
        }
    }
}

pub(crate) struct EbuildRaw {
    repo: &'static Repo,
    filters: PkgFilters,
}

impl EbuildRaw {
    pub(crate) fn new(repo: &'static Repo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            repo,
            filters: PkgFilters(filters),
        }
    }
}

impl IterRestrict for EbuildRaw {
    type Item = ebuild::raw::Pkg<'static>;

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> Box<dyn Iterator<Item = Self::Item> + '_> {
        if self.filters.is_empty() {
            Box::new(self.repo.iter_raw_restrict(val))
        } else {
            Box::new(
                self.filters
                    .iter_restrict(self.repo, val)
                    .flat_map(|pkg| self.repo.iter_raw_restrict(&pkg)),
            )
        }
    }
}
