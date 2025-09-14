use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, Not};
use std::str::FromStr;
use std::sync::{Arc, LazyLock};

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Itertools;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use pkgcraft::repo::{EbuildRepo, Repository};
use pkgcraft::restrict::Scope;
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, Display, EnumIter, EnumString};

use crate::Error;
use crate::report::ReportKind;
use crate::scan::ScannerRun;
use crate::source::SourceKind;

mod commands;
mod dependency;
mod dependency_slot_missing;
mod duplicates;
mod eapi_stale;
mod eapi_status;
mod ebuild_name;
mod eclass;
mod filesdir;
mod header;
mod homepage;
mod ignore;
mod iuse;
mod keywords;
mod keywords_dropped;
mod license;
mod live;
mod manifest;
mod metadata;
mod properties;
mod python_update;
mod repo_layout;
mod restrict;
mod restrict_test_missing;
mod ruby_update;
mod src_uri;
mod unstable_only;
mod use_local;
mod variable_order;
mod variables;
mod whitespace;

/// Check variants.
#[derive(
    AsRefStr,
    Display,
    EnumIter,
    EnumString,
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
    Commands,
    Dependency,
    DependencySlotMissing,
    Duplicates,
    EapiStale,
    EapiStatus,
    EbuildName,
    Eclass,
    Filesdir,
    Header,
    Homepage,
    Ignore,
    Iuse,
    Keywords,
    KeywordsDropped,
    License,
    Live,
    Manifest,
    Metadata,
    Properties,
    PythonUpdate,
    RepoLayout,
    Restrict,
    RestrictTestMissing,
    RubyUpdate,
    SrcUri,
    UnstableOnly,
    UseLocal,
    VariableOrder,
    Variables,
    Whitespace,
}

impl From<CheckKind> for Check {
    fn from(value: CheckKind) -> Self {
        CHECKS
            .get(&value)
            .copied()
            .unwrap_or_else(|| panic!("no registered check: {value}"))
    }
}

/// Registered check.
#[derive(Debug, Copy, Clone)]
pub struct Check {
    pub kind: CheckKind,
    pub reports: &'static [ReportKind],
    pub(crate) scope: Scope,
    pub(crate) sources: &'static [SourceKind],
    pub context: &'static [Context],
    create: fn(&ScannerRun) -> Runner,
}

impl Check {
    /// Create a check runner from a check.
    pub(crate) fn to_runner(self, run: &ScannerRun) -> CheckRunner {
        CheckRunner {
            check: self,
            runner: (self.create)(run),
        }
    }

    /// Return an iterator of available checks.
    pub fn iter() -> impl Iterator<Item = Self> {
        CHECKS.iter().copied()
    }

    /// Return an iterator of checks enabled by default for a full repo scan.
    pub fn iter_default(repo: &EbuildRepo) -> impl Iterator<Item = Check> + '_ {
        let selected = Default::default();
        Self::iter().filter(move |x| x.skipped(repo, &selected).is_none())
    }

    /// Return an iterator of all checks that can be run on a repo at an optional scope.
    pub fn iter_supported<T: Into<Scope>>(
        repo: &EbuildRepo,
        value: T,
    ) -> impl Iterator<Item = Check> + '_ {
        let scope = value.into();
        let selected = Self::iter().collect();
        Self::iter().filter(move |x| x.skipped(repo, &selected).is_none() && scope >= x.scope)
    }

    /// Return an iterator of checks that generate target reports.
    pub fn iter_report<'a, I>(reports: I) -> impl Iterator<Item = Check> + 'a
    where
        I: IntoIterator<Item = &'a ReportKind>,
        I::IntoIter: 'a,
    {
        reports
            .into_iter()
            .filter_map(|x| REPORTS.get(x))
            .flatten()
            .copied()
    }

    /// Return an iterator of checks that use a given source.
    pub fn iter_source(source: &SourceKind) -> impl Iterator<Item = Check> + '_ {
        Self::iter().filter(move |c| c.sources.contains(source))
    }

    /// Determine if a check is skipped for a scanning run due to scan context.
    pub(crate) fn skipped(
        &self,
        repo: &EbuildRepo,
        selected: &IndexSet<Self>,
    ) -> Option<Context> {
        self.context.iter().copied().find(|context| {
            match context {
                Context::Gentoo => repo.name() == "gentoo" || selected.contains(self),
                Context::GentooInherited => repo.trees().any(|x| x.name() == "gentoo"),
                Context::Optional => selected.contains(self),
                Context::Overlay => !repo.masters().is_empty(),
            }
            .not()
        })
    }

    /// Determine if a check is disabled for a scanning run due to package filtering.
    pub(crate) fn filtered(&self) -> bool {
        self.scope != Scope::Version
            || (!self.sources.contains(&SourceKind::EbuildPkg)
                && !self.sources.contains(&SourceKind::EbuildRawPkg))
    }

    /// Determine if a check is disabled for a scanning run due to scan scope.
    pub(crate) fn scoped(&self, scope: Scope) -> Option<Scope> {
        if self.scope > scope {
            Some(self.scope)
        } else {
            None
        }
    }

    /// Check requires post-run finalization for a scope.
    pub(crate) fn finish_check(&self, scope: Scope) -> bool {
        self.reports.iter().any(|r| r.finish_check(scope))
    }

    /// Check requires post-run target finalization.
    pub(crate) fn finish_target(&self) -> bool {
        self.reports.iter().any(|r| r.finish_target())
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
        self.kind.hash(state);
    }
}

impl Borrow<CheckKind> for Check {
    fn borrow(&self) -> &CheckKind {
        &self.kind
    }
}

impl Ord for Check {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind.cmp(&other.kind)
    }
}

impl PartialOrd for Check {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl FromStr for Check {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kind: CheckKind = s
            .parse()
            .map_err(|_| Error::InvalidValue(format!("unknown check: {s}")))?;

        Ok(CHECKS.get(&kind).copied().unwrap())
    }
}

impl AsRef<Utf8Path> for Check {
    fn as_ref(&self) -> &Utf8Path {
        Utf8Path::new(self.kind.as_ref())
    }
}

inventory::collect!(Check);

/// The ordered set of all checks.
static CHECKS: LazyLock<IndexSet<Check>> =
    LazyLock::new(|| inventory::iter::<Check>().copied().sorted().collect());

/// Context required to operate by check or report.
#[derive(
    Debug, Display, EnumIter, EnumString, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
#[strum(serialize_all = "kebab-case")]
pub enum Context {
    /// Check only runs by default in the gentoo repo.
    Gentoo,

    /// Check only runs in repos inheriting from the gentoo repo.
    GentooInherited,

    /// Check isn't enabled by default.
    Optional,

    /// Check only runs in overlay repos.
    Overlay,
}

/// Register a check.
macro_rules! register {
    ($check:expr) => {
        static CHECK: $crate::check::Check = $check;
        inventory::submit! { CHECK }

        impl std::fmt::Display for Check {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{CHECK}")
            }
        }
    };
}
use register;

/// Check running machinery.
#[allow(unused_variables)]
pub(crate) trait CheckRun {
    // repo support
    fn run_repo(&self, run: &ScannerRun) {}

    // category support
    fn run_category(&self, category: &str, run: &ScannerRun) {}
    fn finish_category(&self, category: &str, run: &ScannerRun) {}

    // Cpv support
    fn run_cpv(&self, cpv: &Cpv, run: &ScannerRun) {}
    fn finish_cpv(&self, cpv: &Cpv, run: &ScannerRun) {}

    // Cpn support
    fn run_cpn(&self, cpn: &Cpn, run: &ScannerRun) {}
    fn finish_cpn(&self, cpn: &Cpn, run: &ScannerRun) {}

    // ebuild pkg support
    fn run_ebuild_pkg(&self, pkg: &EbuildPkg, run: &ScannerRun) {}
    fn run_ebuild_pkg_set(&self, cpn: &Cpn, pkgs: &[EbuildPkg], run: &ScannerRun) {}

    // raw ebuild pkg support
    fn run_ebuild_raw_pkg(&self, pkg: &EbuildRawPkg, run: &ScannerRun) {}
    fn run_ebuild_raw_pkg_set(&self, cpn: &Cpn, pkgs: &[EbuildRawPkg], run: &ScannerRun) {}

    // finalization support
    fn finish_check(&self, run: &ScannerRun) {}
}

type Runner = Box<dyn CheckRun + Send + Sync>;

/// Wrapper for running checks.
pub(crate) struct CheckRunner {
    pub(crate) check: Check,
    runner: Runner,
}

impl Deref for CheckRunner {
    type Target = Runner;

    fn deref(&self) -> &Self::Target {
        &self.runner
    }
}

impl fmt::Display for CheckRunner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.check)
    }
}

impl PartialEq for CheckRunner {
    fn eq(&self, other: &Self) -> bool {
        self.check == other.check
    }
}

impl Eq for CheckRunner {}

impl Hash for CheckRunner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.check.hash(state);
    }
}

impl Borrow<Check> for CheckRunner {
    fn borrow(&self) -> &Check {
        &self.check
    }
}

impl Borrow<Check> for Arc<CheckRunner> {
    fn borrow(&self) -> &Check {
        &self.check
    }
}

impl Ord for CheckRunner {
    fn cmp(&self, other: &Self) -> Ordering {
        self.check.cmp(&other.check)
    }
}

impl PartialOrd for CheckRunner {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The mapping of all report variants to the checks that can generate them.
static REPORTS: LazyLock<OrderedMap<ReportKind, OrderedSet<Check>>> = LazyLock::new(|| {
    Check::iter()
        .flat_map(|c| c.reports.iter().copied().map(move |r| (r, c)))
        .collect()
});

#[cfg(test)]
mod tests {
    use pkgcraft::test::assert_ordered_eq;
    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn kind() {
        // verify checks are registered in lexical order
        let kinds: Vec<_> = Check::iter().collect();
        let ordered: Vec<_> = Check::iter().map(|x| x.to_string()).sorted().collect();
        let ordered: Vec<_> = ordered.iter().map(|s| s.parse().unwrap()).collect();
        assert_ordered_eq!(&kinds, &ordered);
    }

    #[test]
    fn report() {
        // verify all report variants have at least one check
        let reports: Vec<_> = ReportKind::iter()
            .filter(|x| REPORTS.get(x).is_none())
            .collect();
        assert!(reports.is_empty(), "no checks for reports: {}", reports.iter().join(", "));
    }

    // TODO: re-enable test when a SourceKind::Repo check is implemented
    /*#[test]
    fn source() {
        // verify all source variants have at least one check
        let sources: Vec<_> = SourceKind::iter()
            .filter(|x| Check::iter_source(x).next().is_none())
            .collect();
        assert!(sources.is_empty(), "no checks for sources: {}", sources.iter().join(", "));
    }*/
}
