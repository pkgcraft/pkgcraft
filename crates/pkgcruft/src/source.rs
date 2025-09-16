use std::fmt;
use std::str::FromStr;

use colored::{Color, Colorize};
use indexmap::{IndexMap, IndexSet};
use itertools::{Either, Itertools};
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::error::Error::InvalidPkg;
use pkgcraft::pkg::Package;
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg, keyword::KeywordStatus};
use pkgcraft::repo::PkgRepository;
use pkgcraft::restrict::{self, Restrict, Restriction, Scope};
use pkgcraft::types::OrderedMap;
use strum::{AsRefStr, Display, EnumIter, IntoEnumIterator};

use crate::check::{CheckRun, CheckRunner};
use crate::error::Error;
use crate::scan::ScannerRun;

/// All check runner source variants.
#[derive(Display, EnumIter, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
pub enum SourceKind {
    Cpv,
    EbuildPkg,
    EbuildRawPkg,
    Cpn,
    Category,
    Repo,
}

impl SourceKind {
    /// Return the source scope.
    pub(crate) fn scope(&self) -> Scope {
        match self {
            Self::Cpv => Scope::Version,
            Self::EbuildPkg => Scope::Version,
            Self::EbuildRawPkg => Scope::Version,
            Self::Cpn => Scope::Package,
            Self::Category => Scope::Category,
            Self::Repo => Scope::Repo,
        }
    }
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

impl PkgFilter {
    /// Apply filter across an iterator of packages.
    fn filter<'a>(
        &'a self,
        iter: Box<dyn Iterator<Item = EbuildPkg> + 'a>,
    ) -> Box<dyn Iterator<Item = EbuildPkg> + 'a> {
        match self {
            Self::Latest(inverted) => {
                let items: Vec<_> = iter.collect();
                let len = items.len();
                if items.is_empty() {
                    Box::new(items.into_iter())
                } else if *inverted {
                    Box::new(items.into_iter().take(len - 1))
                } else {
                    Box::new(items.into_iter().skip(len - 1))
                }
            }
            Self::LatestSlots(inverted) => Box::new(
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
            Self::Live(inverted) => Box::new(iter.filter(move |pkg| inverted ^ pkg.live())),
            Self::Masked(inverted) => {
                Box::new(iter.filter(move |pkg| inverted ^ pkg.masked()))
            }
            Self::Stable(inverted) => {
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
            Self::Restrict(inverted, restrict) => {
                Box::new(iter.filter(move |pkg| inverted ^ restrict.matches(pkg)))
            }
        }
    }
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
            s if s.contains(|c: char| c.is_whitespace()) => {
                Ok(restrict::parse::pkg(s).map(|r| Self::Restrict(inverted, r))?)
            }
            s => {
                let possible = Self::iter()
                    .filter(|r| !matches!(r, Self::Restrict(_, _)))
                    .map(|r| r.as_ref().color(Color::Green))
                    .join(", ");
                let message = indoc::formatdoc! {r#"
                    invalid filter: {s}
                      [possible values: {possible}]

                    Custom restrictions are supported, for example to target all packages
                    maintained by the python project use the following command:

                    pkgcruft scan -f "maintainers any email == 'python@gentoo.org'""#};
                Err(Error::InvalidValue(message))
            }
        }
    }
}

/// Layered package filtering support.
#[derive(Debug, PartialEq, Eq, Clone)]
struct PkgFilters<'a>(&'a IndexSet<PkgFilter>);

impl<'a> PkgFilters<'a> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn iter_restrict<R: Into<Restrict>>(
        &self,
        run: &ScannerRun,
        val: R,
    ) -> Box<dyn Iterator<Item = EbuildPkg> + 'a> {
        let mut iter: Box<dyn Iterator<Item = EbuildPkg>> =
            Box::new(run.repo.iter_restrict(val).filter_map(Result::ok));

        for f in self.0 {
            iter = f.filter(iter);
        }

        iter
    }

    fn iter_restrict_ordered<R: Into<Restrict>>(
        &self,
        run: &ScannerRun,
        val: R,
    ) -> Box<dyn Iterator<Item = EbuildPkg> + 'a> {
        let mut iter: Box<dyn Iterator<Item = EbuildPkg>> =
            Box::new(run.repo.iter_restrict_ordered(val).filter_map(Result::ok));

        for f in self.0 {
            iter = f.filter(iter);
        }

        iter
    }
}

pub(crate) trait Source: fmt::Display {
    type Item;

    /// Return the [`SourceKind`] for the source.
    fn kind(&self) -> SourceKind;

    /// Return the iterator of items matching a restriction.
    fn iter_restrict<'a, R: Into<Restrict>>(
        &self,
        run: &'a ScannerRun,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + 'a;

    /// Return the parallelized, ordered iterator of items matching a restriction.
    fn iter_restrict_ordered<'a, R: Into<Restrict>>(
        &self,
        run: &'a ScannerRun,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + 'a;
}

pub(crate) trait PkgSource {
    type Pkg;

    /// Run all checks for a pkg.
    fn run_pkg(&self, runner: &CheckRunner, pkg: &Self::Pkg, run: &ScannerRun);

    /// Run all checks for a set of pkgs.
    fn run_pkg_set(
        &self,
        runner: &CheckRunner,
        cpn: &Cpn,
        pkgs: &[Self::Pkg],
        run: &ScannerRun,
    );
}

/// All check runner source variants.
#[derive(Display, EnumIter, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
#[strum(serialize_all = "kebab-case")]
pub(crate) enum Pkg {
    EbuildPkg,
    EbuildRawPkg,
}

#[derive(Default)]
pub(crate) struct EbuildPkgSource;

impl fmt::Display for EbuildPkgSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl Source for EbuildPkgSource {
    type Item = pkgcraft::Result<EbuildPkg>;

    fn kind(&self) -> SourceKind {
        SourceKind::EbuildPkg
    }

    fn iter_restrict<'a, R: Into<Restrict>>(
        &self,
        run: &'a ScannerRun,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + 'a {
        let filters = PkgFilters(&run.filters);
        if !filters.is_empty() {
            Either::Left(
                filters
                    .iter_restrict(run, val)
                    .flat_map(|pkg| run.repo.iter_restrict(&pkg)),
            )
        } else {
            Either::Right(run.repo.iter_restrict(val))
        }
    }

    fn iter_restrict_ordered<'a, R: Into<Restrict>>(
        &self,
        run: &'a ScannerRun,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + 'a {
        let filters = PkgFilters(&run.filters);
        if !filters.is_empty() {
            Either::Left(
                filters
                    .iter_restrict_ordered(run, val)
                    .flat_map(|pkg| run.repo.iter_restrict_ordered(&pkg)),
            )
        } else {
            Either::Right(run.repo.iter_restrict_ordered(val))
        }
    }
}

impl PkgSource for EbuildPkgSource {
    type Pkg = EbuildPkg;

    fn run_pkg(&self, runner: &CheckRunner, pkg: &Self::Pkg, run: &ScannerRun) {
        runner.run_ebuild_pkg(pkg, run)
    }

    fn run_pkg_set(
        &self,
        runner: &CheckRunner,
        cpn: &Cpn,
        pkgs: &[Self::Pkg],
        run: &ScannerRun,
    ) {
        runner.run_ebuild_pkg_set(cpn, pkgs, run)
    }
}

#[derive(Default)]
pub(crate) struct EbuildRawPkgSource;

impl fmt::Display for EbuildRawPkgSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl Source for EbuildRawPkgSource {
    type Item = pkgcraft::Result<EbuildRawPkg>;

    fn kind(&self) -> SourceKind {
        SourceKind::EbuildRawPkg
    }

    fn iter_restrict<'a, R: Into<Restrict>>(
        &self,
        run: &'a ScannerRun,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + 'a {
        let filters = PkgFilters(&run.filters);
        if !filters.is_empty() {
            Either::Left(
                filters
                    .iter_restrict(run, val)
                    .flat_map(|pkg| run.repo.iter_raw_restrict(&pkg)),
            )
        } else {
            Either::Right(run.repo.iter_raw_restrict(val))
        }
    }

    fn iter_restrict_ordered<'a, R: Into<Restrict>>(
        &self,
        run: &'a ScannerRun,
        val: R,
    ) -> impl Iterator<Item = Self::Item> + 'a {
        let filters = PkgFilters(&run.filters);
        if !filters.is_empty() {
            Either::Left(
                filters
                    .iter_restrict_ordered(run, val)
                    .flat_map(|pkg| run.repo.iter_raw_restrict_ordered(&pkg)),
            )
        } else {
            Either::Right(run.repo.iter_raw_restrict_ordered(val))
        }
    }
}

impl PkgSource for EbuildRawPkgSource {
    type Pkg = EbuildRawPkg;

    fn run_pkg(&self, runner: &CheckRunner, pkg: &Self::Pkg, run: &ScannerRun) {
        runner.run_ebuild_raw_pkg(pkg, run)
    }

    fn run_pkg_set(
        &self,
        runner: &CheckRunner,
        cpn: &Cpn,
        pkgs: &[Self::Pkg],
        run: &ScannerRun,
    ) {
        runner.run_ebuild_raw_pkg_set(cpn, pkgs, run)
    }
}

/// Cache used to avoid recreating package objects for package and version scope scans.
#[derive(Debug)]
pub(crate) struct PkgCache<T> {
    pkgs: pkgcraft::Result<Vec<T>>,
    cache: IndexMap<Cpv, pkgcraft::Result<T>>,
}

impl<T: Package + Clone> PkgCache<T> {
    /// Create a new package cache from a source and restriction.
    pub(crate) fn new<S>(source: &S, run: &ScannerRun) -> Self
    where
        S: Source<Item = pkgcraft::Result<T>>,
    {
        let mut cache = IndexMap::new();

        for result in source.iter_restrict_ordered(run, &run.restrict) {
            if let Ok(pkg) = &result {
                cache.insert(pkg.cpv().clone(), result);
            } else if let Err(InvalidPkg { cpv, .. }) = &result {
                cache.insert(*cpv.clone(), result);
            }
        }

        Self {
            pkgs: cache.values().cloned().try_collect(),
            cache,
        }
    }

    /// Get all packages from the cache if none were invalid on creation.
    pub(crate) fn get_pkgs(&self) -> Result<&[T], &pkgcraft::Error> {
        self.pkgs.as_deref()
    }

    /// Get a matching package result from the cache if it exists.
    pub(crate) fn get_pkg(&self, cpv: &Cpv) -> Option<&pkgcraft::Result<T>> {
        self.cache.get(cpv)
    }
}
