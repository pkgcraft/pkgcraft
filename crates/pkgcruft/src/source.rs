use std::str::FromStr;

use colored::{Color, Colorize};
use indexmap::IndexSet;
use itertools::{Either, Itertools};
use pkgcraft::pkg::ebuild::keyword::KeywordStatus;
use pkgcraft::pkg::ebuild::{self, EbuildPackage};
use pkgcraft::repo::ebuild::EbuildRepo;
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::{self, Restrict, Restriction};
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
    EbuildPkg,
    EbuildRawPkg,
    UnversionedPkg,
}

/// Package filtering variants.
#[derive(AsRefStr, EnumIter, Debug, PartialEq, Eq, Hash, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum PkgFilter {
    /// Filter packages using the latest version only.
    Latest(bool),

    /// Filter packages using the latest version from each slot.
    LatestSlots(bool),

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
            "latest" => Ok(Self::Latest(inverted)),
            "latest-slots" => Ok(Self::LatestSlots(inverted)),
            "live" => Ok(Self::Live(inverted)),
            "masked" => Ok(Self::Masked(inverted)),
            "stable" => Ok(Self::Stable(inverted)),
            s if s.contains(|c: char| c.is_whitespace()) => restrict::parse::pkg(s)
                .map(|r| Self::Restrict(inverted, r))
                .map_err(|e| Error::InvalidValue(format!("{e}"))),
            s => {
                let possible = Self::iter()
                    .filter(|r| !matches!(r, Self::Restrict(_, _)))
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
        repo: &'static EbuildRepo,
        val: R,
    ) -> Box<dyn Iterator<Item = ebuild::Pkg> + '_> {
        let mut iter: Box<dyn Iterator<Item = ebuild::Pkg>> = Box::new(repo.iter_restrict(val));

        for filter in &self.0 {
            iter = match filter {
                PkgFilter::Latest(inverted) => {
                    let items: Vec<_> = iter.collect();
                    let len = items.len();
                    if *inverted {
                        Box::new(items.into_iter().take(len - 1))
                    } else {
                        Box::new(items.into_iter().skip(len - 1))
                    }
                }
                PkgFilter::LatestSlots(inverted) => Box::new(
                    iter.map(|pkg| (pkg.slot().to_string(), pkg))
                        .collect::<OrderedMap<_, Vec<_>>>()
                        .into_values()
                        .flat_map(|pkgs| {
                            let len = pkgs.len();
                            if *inverted {
                                Either::Left(pkgs.into_iter().take(len - 1))
                            } else {
                                Either::Right(pkgs.into_iter().skip(len - 1))
                            }
                        }),
                ),
                PkgFilter::Live(inverted) => {
                    Box::new(iter.filter(move |pkg| inverted ^ pkg.live()))
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

pub(crate) struct EbuildPkg {
    repo: &'static EbuildRepo,
    filters: PkgFilters,
}

impl EbuildPkg {
    pub(crate) fn new(repo: &'static EbuildRepo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            repo,
            filters: PkgFilters(filters),
        }
    }
}

impl IterRestrict for EbuildPkg {
    type Item = ebuild::Pkg;

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        val: R,
    ) -> Box<dyn Iterator<Item = Self::Item> + '_> {
        self.filters.iter_restrict(self.repo, val)
    }
}

pub(crate) struct EbuildRawPkg {
    repo: &'static EbuildRepo,
    filters: PkgFilters,
}

impl EbuildRawPkg {
    pub(crate) fn new(repo: &'static EbuildRepo, filters: IndexSet<PkgFilter>) -> Self {
        Self {
            repo,
            filters: PkgFilters(filters),
        }
    }
}

impl IterRestrict for EbuildRawPkg {
    type Item = ebuild::raw::Pkg;

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
