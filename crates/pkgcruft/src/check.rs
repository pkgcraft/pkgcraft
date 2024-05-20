use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use indexmap::IndexSet;
use once_cell::sync::Lazy;
use pkgcraft::macros::cmp_not_equal;
use pkgcraft::pkg::ebuild;
use pkgcraft::repo::ebuild::Repo;
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};

use crate::report::{Report, ReportKind};
use crate::scope::Scope;
use crate::source::SourceKind;

pub mod dependency;
pub mod dropped_keywords;
pub mod eapi;
pub mod keywords;
pub mod metadata;
pub mod unstable_only;

/// All checks separated by source type.
#[derive(Debug, Copy, Clone)]
pub(crate) enum CheckValue {
    Pkg,
    RawPkg,
    PkgSet,
}

/// All checks separated by source type.
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
pub enum CheckKind {
    Dependency,
    DroppedKeywords,
    Eapi,
    Keywords,
    Metadata,
    UnstableOnly,
}

impl CheckKind {
    pub fn check(self) -> &'static Check {
        match self {
            Self::Dependency => &dependency::CHECK,
            Self::DroppedKeywords => &dropped_keywords::CHECK,
            Self::Eapi => &eapi::CHECK,
            Self::Keywords => &keywords::CHECK,
            Self::Metadata => &metadata::CHECK,
            Self::UnstableOnly => &unstable_only::CHECK,
        }
    }

    pub(crate) fn value(&self) -> CheckValue {
        match self {
            Self::Dependency => CheckValue::Pkg,
            Self::DroppedKeywords => CheckValue::PkgSet,
            Self::Eapi => CheckValue::Pkg,
            Self::Keywords => CheckValue::Pkg,
            Self::Metadata => CheckValue::RawPkg,
            Self::UnstableOnly => CheckValue::PkgSet,
        }
    }

    pub(crate) fn ebuild(self, repo: &Repo) -> EbuildPkgCheck {
        use EbuildPkgCheck::*;
        match self {
            Self::Dependency => Dependency(dependency::Check::new(repo)),
            Self::Eapi => Eapi(eapi::Check::new(repo)),
            Self::Keywords => Keywords(keywords::Check::new(repo)),
            _ => unreachable!("{self} is not an ebuild check"),
        }
    }

    pub(crate) fn ebuild_raw(self, repo: &Repo) -> EbuildRawPkgCheck {
        use EbuildRawPkgCheck::*;
        match self {
            Self::Metadata => Metadata(metadata::Check::new(repo)),
            _ => unreachable!("{self} is not a raw ebuild check"),
        }
    }

    pub(crate) fn ebuild_pkg_set(self, repo: &Repo) -> EbuildPkgSetCheck {
        use EbuildPkgSetCheck::*;
        match self {
            Self::DroppedKeywords => DroppedKeywords(dropped_keywords::Check::new(repo)),
            Self::UnstableOnly => UnstableOnly(unstable_only::Check::new(repo)),
            _ => unreachable!("{self} is not an ebuild pkg set check"),
        }
    }
}

#[derive(Debug)]
pub(crate) enum EbuildPkgCheck<'a> {
    Dependency(dependency::Check<'a>),
    Eapi(eapi::Check<'a>),
    Keywords(keywords::Check<'a>),
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
    Metadata(metadata::Check<'a>),
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
    DroppedKeywords(dropped_keywords::Check<'a>),
    UnstableOnly(unstable_only::Check<'a>),
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

#[derive(Debug)]
struct CheckBuilder(Check);

impl CheckBuilder {
    /// Create a new Check builder for a given variant.
    fn new(kind: CheckKind) -> Self {
        Self(Check {
            kind,
            source: Default::default(),
            scope: Default::default(),
            priority: Default::default(),
            reports: Default::default(),
        })
    }

    fn scope(mut self, value: Scope) -> Self {
        self.0.scope = value;
        self
    }

    fn source(mut self, value: SourceKind) -> Self {
        self.0.source = value;
        self
    }

    fn priority(mut self, value: i64) -> Self {
        self.0.priority = value;
        self
    }

    fn reports<I>(mut self, values: I) -> Check
    where
        I: IntoIterator<Item = ReportKind>,
    {
        self.0.reports = values.into_iter().collect();
        self.0
    }
}

#[derive(Debug)]
pub struct Check {
    /// The check variant.
    pub kind: CheckKind,

    /// The scope the check runs in.
    pub scope: Scope,

    /// The source of the values the check runs against.
    pub source: SourceKind,

    /// The priority of the check for enabling a deterministic running order.
    priority: i64,

    /// All the potential report variants generated by the check.
    pub reports: IndexSet<ReportKind>,
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

impl Borrow<str> for &'static Check {
    fn borrow(&self) -> &str {
        self.kind.as_ref()
    }
}

impl Borrow<CheckKind> for &'static Check {
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

impl From<CheckKind> for &'static Check {
    fn from(kind: CheckKind) -> Self {
        kind.check()
    }
}

impl std::fmt::Display for Check {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

/// The ordered map of all report variants to the checks that can generate them.
pub static REPORT_CHECKS: Lazy<OrderedMap<ReportKind, OrderedSet<CheckKind>>> = Lazy::new(|| {
    let mut map: OrderedMap<_, OrderedSet<_>> = CheckKind::iter()
        .flat_map(|c| c.check().reports.iter().copied().map(move |r| (r, c)))
        .collect();
    map.sort_keys();
    map
});

/// The ordered map of all source variants to the checks that use them.
pub static SOURCE_CHECKS: Lazy<OrderedMap<SourceKind, OrderedSet<CheckKind>>> = Lazy::new(|| {
    let mut map: OrderedMap<_, OrderedSet<_>> =
        CheckKind::iter().map(|c| (c.check().source, c)).collect();
    map.sort_keys();
    map
});
