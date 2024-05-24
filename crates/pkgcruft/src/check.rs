use std::cmp::Ordering;

use camino::Utf8Path;
use once_cell::sync::Lazy;
use pkgcraft::macros::cmp_not_equal;
use pkgcraft::pkg::ebuild;
use pkgcraft::repo::{ebuild::Repo, Repository};
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};

use crate::report::{Report, ReportKind};
use crate::scope::Scope;
use crate::source::SourceKind;

mod dependency;
mod dropped_keywords;
mod eapi;
mod eapi_stale;
mod keywords;
mod metadata;
mod missing_slot_dep;
mod missing_test_restrict;
mod unstable_only;

/// Check contexts.
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
pub enum CheckContext {
    Gentoo,
    Optional,
    Overlay,
}

impl CheckContext {
    /// Determine if a context is enabled.
    pub(super) fn enabled(&self, repo: &Repo) -> bool {
        match self {
            Self::Gentoo => repo.name() == "gentoo",
            Self::Optional => false,
            Self::Overlay => repo.masters().next().is_some(),
        }
    }
}

/// Check variants.
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
pub enum CheckKind {
    Dependency,
    DroppedKeywords,
    Eapi,
    EapiStale,
    Keywords,
    Metadata,
    MissingSlotDep,
    MissingTestRestrict,
    UnstableOnly,
}

impl AsRef<Utf8Path> for CheckKind {
    fn as_ref(&self) -> &Utf8Path {
        let s: &str = self.as_ref();
        Utf8Path::new(s)
    }
}

impl CheckKind {
    /// The priority of the check for enabling a deterministic running order.
    fn priority(&self) -> i64 {
        match self {
            Self::Metadata => -9999,
            _ => 0,
        }
    }

    /// Compare check variants by priority, then by name.
    pub(crate) fn prioritized(left: &Self, right: &Self) -> Ordering {
        cmp_not_equal!(&left.priority(), &right.priority());
        left.cmp(right)
    }

    /// The scope the check runs in.
    pub fn scope(&self) -> Scope {
        match self {
            Self::Dependency => Scope::Version,
            Self::DroppedKeywords => Scope::Package,
            Self::Eapi => Scope::Version,
            Self::EapiStale => Scope::Package,
            Self::Keywords => Scope::Version,
            Self::Metadata => Scope::Version,
            Self::MissingSlotDep => Scope::Version,
            Self::MissingTestRestrict => Scope::Version,
            Self::UnstableOnly => Scope::Package,
        }
    }

    /// The source of the values the check runs against.
    pub fn source(&self) -> SourceKind {
        match self {
            Self::Dependency => SourceKind::Ebuild,
            Self::DroppedKeywords => SourceKind::Ebuild,
            Self::Eapi => SourceKind::Ebuild,
            Self::EapiStale => SourceKind::Ebuild,
            Self::Keywords => SourceKind::Ebuild,
            Self::Metadata => SourceKind::EbuildRaw,
            Self::MissingSlotDep => SourceKind::Ebuild,
            Self::MissingTestRestrict => SourceKind::Ebuild,
            Self::UnstableOnly => SourceKind::Ebuild,
        }
    }

    /// All the potential report variants generated by the check.
    pub fn reports(self) -> &'static [ReportKind] {
        match self {
            Self::Dependency => dependency::REPORTS,
            Self::DroppedKeywords => dropped_keywords::REPORTS,
            Self::Eapi => eapi::REPORTS,
            Self::EapiStale => eapi_stale::REPORTS,
            Self::Keywords => keywords::REPORTS,
            Self::Metadata => metadata::REPORTS,
            Self::MissingSlotDep => missing_slot_dep::REPORTS,
            Self::MissingTestRestrict => missing_test_restrict::REPORTS,
            Self::UnstableOnly => unstable_only::REPORTS,
        }
    }

    /// Create a check runner for a given variant.
    #[rustfmt::skip]
    pub(crate) fn create(self, repo: &Repo) -> Check {
        use Check::*;
        match self {
            Self::Dependency => Dependency(dependency::Check::new(repo)),
            Self::DroppedKeywords => DroppedKeywords(dropped_keywords::Check::new(repo)),
            Self::Eapi => Eapi(eapi::Check::new(repo)),
            Self::EapiStale => EapiStale(eapi_stale::Check::new(repo)),
            Self::Keywords => Keywords(keywords::Check::new(repo)),
            Self::Metadata => Metadata(metadata::Check::new(repo)),
            Self::MissingSlotDep => MissingSlotDep(missing_slot_dep::Check::new(repo)),
            Self::MissingTestRestrict => MissingTestRestrict(missing_test_restrict::Check::new(repo)),
            Self::UnstableOnly => UnstableOnly(unstable_only::Check::new(repo)),
        }
    }

    /// Check variant contexts.
    pub(crate) fn context(&self) -> &[CheckContext] {
        use CheckContext::*;
        match self {
            Self::UnstableOnly => &[Gentoo],
            _ => &[],
        }
    }
}

/// Check runner variants.
#[deny(dead_code)]
#[derive(AsRefStr, Display, Debug)]
#[strum(serialize_all = "kebab-case")]
pub(crate) enum Check<'a> {
    Dependency(dependency::Check<'a>),
    DroppedKeywords(dropped_keywords::Check<'a>),
    Eapi(eapi::Check<'a>),
    EapiStale(eapi_stale::Check<'a>),
    Keywords(keywords::Check<'a>),
    Metadata(metadata::Check<'a>),
    MissingSlotDep(missing_slot_dep::Check<'a>),
    MissingTestRestrict(missing_test_restrict::Check<'a>),
    UnstableOnly(unstable_only::Check<'a>),
}

impl<'a> Check<'a> {
    pub(crate) fn kind(&self) -> CheckKind {
        self.as_ref()
            .parse()
            .unwrap_or_else(|_| panic!("{self} name doesn't match CheckKind"))
    }
}

impl<'a> CheckRun<&ebuild::Pkg<'a>> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkg: &ebuild::Pkg<'a>, report: F) {
        match self {
            Self::Dependency(c) => c.run(pkg, report),
            Self::Eapi(c) => c.run(pkg, report),
            Self::Keywords(c) => c.run(pkg, report),
            Self::MissingSlotDep(c) => c.run(pkg, report),
            Self::MissingTestRestrict(c) => c.run(pkg, report),
            _ => unreachable!("{self} is not an ebuild check"),
        }
    }
}

impl<'a> CheckRun<&ebuild::raw::Pkg<'a>> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkg: &ebuild::raw::Pkg<'a>, report: F) {
        match self {
            Self::Metadata(c) => c.run(pkg, report),
            _ => unreachable!("{self} is not a raw ebuild check"),
        }
    }
}

impl<'a> CheckRun<&[ebuild::Pkg<'a>]> for Check<'a> {
    fn run<F: FnMut(Report)>(&self, pkgs: &[ebuild::Pkg<'a>], report: F) {
        match self {
            Self::DroppedKeywords(c) => c.run(pkgs, report),
            Self::EapiStale(c) => c.run(pkgs, report),
            Self::UnstableOnly(c) => c.run(pkgs, report),
            _ => unreachable!("{self} is not an ebuild pkg set check"),
        }
    }
}

/// Run a check for a given item sending back any generated reports.
pub(crate) trait CheckRun<T> {
    fn run<F: FnMut(Report)>(&self, item: T, report: F);
}

/// The mapping of all report variants to the checks that can generate them.
pub static REPORT_CHECKS: Lazy<OrderedMap<ReportKind, OrderedSet<CheckKind>>> = Lazy::new(|| {
    CheckKind::iter()
        .flat_map(|c| c.reports().iter().copied().map(move |r| (r, c)))
        .collect()
});

/// The mapping of all source variants to the checks that use them.
pub static SOURCE_CHECKS: Lazy<OrderedMap<SourceKind, OrderedSet<CheckKind>>> =
    Lazy::new(|| CheckKind::iter().map(|c| (c.source(), c)).collect());
