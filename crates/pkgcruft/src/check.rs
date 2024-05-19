use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use indexmap::IndexSet;
use once_cell::sync::Lazy;
use pkgcraft::macros::cmp_not_equal;
use pkgcraft::pkg::ebuild;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, EnumIter, EnumString};

use crate::report::{Report, ReportKind};
use crate::scope::Scope;
use crate::source::SourceKind;
use crate::Error;

pub mod dependency;
pub mod dropped_keywords;
pub mod eapi;
pub mod keywords;
pub mod metadata;
pub mod unstable_only;

/// Checks run against ebuild packages.
#[derive(
    AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum EbuildPkgCheckKind {
    Dependency,
    Eapi,
    Keywords,
}

impl EbuildPkgCheckKind {
    pub(crate) fn to_check(self, repo: &Repo) -> EbuildPkgCheck {
        match self {
            Self::Dependency => EbuildPkgCheck::Dependency(dependency::DependencyCheck::new(repo)),
            Self::Eapi => EbuildPkgCheck::Eapi(eapi::EapiCheck::new(repo)),
            Self::Keywords => EbuildPkgCheck::Keywords(keywords::KeywordsCheck::new(repo)),
        }
    }
}

/// Checks run against raw ebuild packages.
#[derive(
    AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum EbuildRawPkgCheckKind {
    Metadata,
}

impl EbuildRawPkgCheckKind {
    pub(crate) fn to_check(self, repo: &Repo) -> EbuildRawPkgCheck {
        match self {
            Self::Metadata => EbuildRawPkgCheck::Metadata(metadata::MetadataCheck::new(repo)),
        }
    }
}

/// Checks run against ebuild package sets.
#[derive(
    AsRefStr, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum EbuildPkgSetCheckKind {
    DroppedKeywords,
    UnstableOnly,
}

impl EbuildPkgSetCheckKind {
    pub(crate) fn to_check(self, repo: &Repo) -> EbuildPkgSetCheck {
        match self {
            Self::DroppedKeywords => EbuildPkgSetCheck::DroppedKeywords(
                dropped_keywords::DroppedKeywordsCheck::new(repo),
            ),
            Self::UnstableOnly => {
                EbuildPkgSetCheck::UnstableOnly(unstable_only::UnstableOnlyCheck::new(repo))
            }
        }
    }
}

/// All checks separated by source type.
#[derive(Debug, Copy, Clone)]
pub enum CheckKind {
    EbuildPkg(EbuildPkgCheckKind),
    EbuildRawPkg(EbuildRawPkgCheckKind),
    EbuildPkgSet(EbuildPkgSetCheckKind),
}

impl FromStr for CheckKind {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        CHECKS
            .get(s)
            .map(|c| c.kind())
            .ok_or_else(|| Error::InvalidValue(format!("invalid check variant: {s}")))
    }
}

impl std::fmt::Display for CheckKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::EbuildPkg(k) => write!(f, "{}", k.as_ref()),
            Self::EbuildRawPkg(k) => write!(f, "{}", k.as_ref()),
            Self::EbuildPkgSet(k) => write!(f, "{}", k.as_ref()),
        }
    }
}

impl AsRef<str> for CheckKind {
    fn as_ref(&self) -> &str {
        match self {
            Self::EbuildPkg(k) => k.as_ref(),
            Self::EbuildRawPkg(k) => k.as_ref(),
            Self::EbuildPkgSet(k) => k.as_ref(),
        }
    }
}

impl PartialEq for CheckKind {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl Eq for CheckKind {}

impl Hash for CheckKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl Borrow<str> for CheckKind {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

impl Ord for CheckKind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for CheckKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub(crate) enum EbuildPkgCheck<'a> {
    Dependency(dependency::DependencyCheck<'a>),
    Eapi(eapi::EapiCheck<'a>),
    Keywords(keywords::KeywordsCheck<'a>),
}

impl<'a> CheckRun<&ebuild::Pkg<'a>> for EbuildPkgCheck<'a> {
    fn run(&self, pkg: &ebuild::Pkg<'a>, reports: &mut Vec<Report>) {
        match self {
            Self::Dependency(c) => c.run(pkg, reports),
            Self::Eapi(c) => c.run(pkg, reports),
            Self::Keywords(c) => c.run(pkg, reports),
        }
    }
}

#[derive(Debug)]
pub(crate) enum EbuildRawPkgCheck<'a> {
    Metadata(metadata::MetadataCheck<'a>),
}

impl<'a> CheckRun<&ebuild::raw::Pkg<'a>> for EbuildRawPkgCheck<'a> {
    fn run(&self, pkg: &ebuild::raw::Pkg<'a>, reports: &mut Vec<Report>) {
        match self {
            Self::Metadata(c) => c.run(pkg, reports),
        }
    }
}

#[derive(Debug)]
pub(crate) enum EbuildPkgSetCheck<'a> {
    DroppedKeywords(dropped_keywords::DroppedKeywordsCheck<'a>),
    UnstableOnly(unstable_only::UnstableOnlyCheck<'a>),
}

impl<'a> CheckRun<&[ebuild::Pkg<'a>]> for EbuildPkgSetCheck<'a> {
    fn run(&self, pkgs: &[ebuild::Pkg<'a>], reports: &mut Vec<Report>) {
        match self {
            Self::DroppedKeywords(c) => c.run(pkgs, reports),
            Self::UnstableOnly(c) => c.run(pkgs, reports),
        }
    }
}

/// Run a check for a given item sending back any generated reports.
pub(crate) trait CheckRun<T> {
    fn run(&self, item: T, reports: &mut Vec<Report>);
}

#[derive(Debug, Copy, Clone)]
pub struct Check {
    kind: CheckKind,
    source: SourceKind,
    scope: Scope,
    priority: i64,
    reports: &'static [ReportKind],
}

impl Check {
    /// The check variant.
    pub fn kind(&self) -> CheckKind {
        self.kind
    }

    /// The priority of the check for enabling a deterministic running order.
    pub fn priority(&self) -> i64 {
        self.priority
    }

    /// The source of the values the check runs against.
    pub fn source(&self) -> SourceKind {
        self.source
    }

    /// The scope the check runs in.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// All the potential report variants generated by the check.
    pub fn reports(&self) -> &[ReportKind] {
        self.reports
    }
}

impl PartialEq for Check {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for Check {}

impl Hash for Check {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state)
    }
}

impl AsRef<str> for Check {
    fn as_ref(&self) -> &str {
        self.kind.as_ref()
    }
}

impl Borrow<str> for Check {
    fn borrow(&self) -> &str {
        self.kind.as_ref()
    }
}

impl Borrow<CheckKind> for Check {
    fn borrow(&self) -> &CheckKind {
        &self.kind
    }
}

impl Ord for Check {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_not_equal!(&self.priority, &other.priority);
        self.kind.cmp(&other.kind)
    }
}

impl PartialOrd for Check {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<CheckKind> for Check {
    fn from(kind: CheckKind) -> Self {
        *CHECKS
            .get(&kind)
            .unwrap_or_else(|| panic!("unregistered check: {kind}"))
    }
}

impl std::fmt::Display for Check {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

/// The ordered set of all check variants.
pub static CHECKS: Lazy<IndexSet<Check>> = Lazy::new(|| {
    [
        dependency::CHECK,
        dropped_keywords::CHECK,
        eapi::CHECK,
        keywords::CHECK,
        metadata::CHECK,
        unstable_only::CHECK,
    ]
    .into_iter()
    .collect()
});

/// The ordered map of all source variants to the checks that use them.
pub static SOURCE_CHECKS: Lazy<OrderedMap<SourceKind, OrderedSet<Check>>> = Lazy::new(|| {
    let mut map: OrderedMap<_, OrderedSet<_>> = CHECKS.iter().map(|c| (c.source(), *c)).collect();
    map.sort_keys();
    map
});
