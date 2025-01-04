use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Not;
use std::sync::LazyLock;

use camino::Utf8Path;
use indexmap::IndexSet;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use pkgcraft::repo::{ebuild::EbuildRepo, Repository};
use pkgcraft::restrict::Scope;
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};

use crate::iter::ReportFilter;
use crate::report::ReportKind;
use crate::source::SourceKind;

mod commands;
mod dependency;
mod dependency_slot_missing;
mod duplicates;
mod eapi_stale;
mod eapi_status;
mod ebuild_name;
mod filesdir;
mod header;
mod homepage;
mod iuse;
mod keywords;
mod keywords_dropped;
mod license;
mod live;
mod manifest;
mod metadata;
mod overlay;
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
mod whitespace;

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
pub enum CheckKind {
    Commands,
    Dependency,
    DependencySlotMissing,
    Duplicates,
    EapiStale,
    EapiStatus,
    EbuildName,
    Filesdir,
    Header,
    Homepage,
    Iuse,
    Keywords,
    KeywordsDropped,
    License,
    Live,
    Manifest,
    Metadata,
    Overlay,
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
    Whitespace,
}

impl CheckKind {
    /// Return the report variants for a check.
    pub fn reports(&self) -> &'static [ReportKind] {
        Check::from(*self).reports
    }
}

impl From<CheckKind> for Check {
    fn from(value: CheckKind) -> Self {
        match value {
            CheckKind::Commands => commands::CHECK,
            CheckKind::Dependency => dependency::CHECK,
            CheckKind::DependencySlotMissing => dependency_slot_missing::CHECK,
            CheckKind::Duplicates => duplicates::CHECK,
            CheckKind::EapiStale => eapi_stale::CHECK,
            CheckKind::EapiStatus => eapi_status::CHECK,
            CheckKind::Filesdir => filesdir::CHECK,
            CheckKind::EbuildName => ebuild_name::CHECK,
            CheckKind::Header => header::CHECK,
            CheckKind::Homepage => homepage::CHECK,
            CheckKind::Iuse => iuse::CHECK,
            CheckKind::Keywords => keywords::CHECK,
            CheckKind::KeywordsDropped => keywords_dropped::CHECK,
            CheckKind::License => license::CHECK,
            CheckKind::Live => live::CHECK,
            CheckKind::Manifest => manifest::CHECK,
            CheckKind::Overlay => overlay::CHECK,
            CheckKind::Metadata => metadata::CHECK,
            CheckKind::Properties => properties::CHECK,
            CheckKind::PythonUpdate => python_update::CHECK,
            CheckKind::RepoLayout => repo_layout::CHECK,
            CheckKind::Restrict => restrict::CHECK,
            CheckKind::RestrictTestMissing => restrict_test_missing::CHECK,
            CheckKind::RubyUpdate => ruby_update::CHECK,
            CheckKind::SrcUri => src_uri::CHECK,
            CheckKind::UnstableOnly => unstable_only::CHECK,
            CheckKind::UseLocal => use_local::CHECK,
            CheckKind::VariableOrder => variable_order::CHECK,
            CheckKind::Whitespace => whitespace::CHECK,
        }
    }
}

/// Check contexts.
#[derive(
    Debug, Display, EnumIter, EnumString, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
#[strum(serialize_all = "kebab-case")]
pub enum CheckContext {
    /// Check only runs by default in the gentoo repo.
    Gentoo,

    /// Check only runs in repos inheriting from the gentoo repo.
    GentooInherited,

    /// Check isn't enabled by default.
    Optional,

    /// Check only runs in overlay repos.
    Overlay,
}

/// Implement various traits for a given check type.
macro_rules! register {
    ($x:ty) => {
        impl std::fmt::Display for $x {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{CHECK}")
            }
        }
    };
}
use register;

/// Run a check against a repo.
pub(crate) trait RepoCheck: fmt::Display {
    fn run(&self, repo: &EbuildRepo, filter: &mut ReportFilter);
}
pub(crate) type RepoRunner = Box<dyn RepoCheck + Send + Sync>;

/// Run a check against a Cpv.
pub(crate) trait CpvCheck: fmt::Display {
    fn run(&self, cpv: &Cpv, filter: &mut ReportFilter);
}
pub(crate) type CpvRunner = Box<dyn CpvCheck + Send + Sync>;

/// Run a check against a Cpn.
pub(crate) trait CpnCheck: fmt::Display {
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter);
}
pub(crate) type CpnRunner = Box<dyn CpnCheck + Send + Sync>;

/// Run a check against a given ebuild package version.
pub(crate) trait EbuildPkgCheck: fmt::Display {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &mut ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type EbuildPkgRunner = Box<dyn EbuildPkgCheck + Send + Sync>;

/// Run a check against a given ebuild package set.
pub(crate) trait EbuildPkgSetCheck: fmt::Display {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &mut ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &mut ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type EbuildPkgSetRunner = Box<dyn EbuildPkgSetCheck + Send + Sync>;

/// Run a check against a given raw ebuild package version.
pub(crate) trait EbuildRawPkgCheck: fmt::Display {
    fn run(&self, pkg: &EbuildRawPkg, filter: &mut ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &mut ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type EbuildRawPkgRunner = Box<dyn EbuildRawPkgCheck + Send + Sync>;

/// Run a check against a raw ebuild package set.
pub(crate) trait EbuildRawPkgSetCheck: fmt::Display {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildRawPkg], filter: &mut ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &mut ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type EbuildRawPkgSetRunner = Box<dyn EbuildRawPkgSetCheck + Send + Sync>;

/// Registered check.
#[derive(Copy, Clone)]
pub struct Check {
    /// The check variant.
    pub(crate) kind: CheckKind,

    /// The scope the check runs in.
    pub(crate) scope: Scope,

    /// The source of the values the check runs against.
    pub(crate) source: SourceKind,

    /// All the potential report variants generated by the check.
    pub(crate) reports: &'static [ReportKind],

    /// Check variant contexts.
    pub(crate) context: &'static [CheckContext],
}

impl Check {
    /// Return an iterator of all registered checks.
    pub fn iter() -> impl Iterator<Item = Check> {
        CheckKind::iter().map(Into::into)
    }

    /// Return an iterator of checks enabled by default for a repo.
    pub fn iter_default(repo: &EbuildRepo) -> impl Iterator<Item = Check> + '_ {
        let selected = IndexSet::new();
        Self::iter().filter(move |x| x.skipped(repo, &selected).is_none())
    }

    /// Return an iterator of all checks that can be run on a repo at an optional scope.
    pub fn iter_supported<T: Into<Scope>>(
        repo: &EbuildRepo,
        scope: T,
    ) -> impl Iterator<Item = Check> + '_ {
        let scope = scope.into();
        let selected = Self::iter().collect();
        Self::iter()
            .filter(move |x| x.skipped(repo, &selected).is_none() && x.scoped(scope).is_none())
    }

    /// Return an iterator of checks that generate target reports.
    pub fn iter_report<'a, I>(reports: I) -> impl Iterator<Item = Check> + 'a
    where
        I: IntoIterator<Item = &'a ReportKind>,
        <I as IntoIterator>::IntoIter: 'a,
    {
        reports
            .into_iter()
            .filter_map(|x| REPORTS.get(x))
            .flatten()
            .copied()
    }

    /// Return an iterator of checks that use a given source.
    pub fn iter_source(source: &SourceKind) -> impl Iterator<Item = Check> + '_ {
        Self::iter().filter(move |c| c.source == *source)
    }

    /// Determine if a check is skipped for a scanning run due to scan context.
    pub(crate) fn skipped(
        &self,
        repo: &EbuildRepo,
        selected: &IndexSet<Self>,
    ) -> Option<CheckContext> {
        self.context.iter().copied().find(|context| {
            match context {
                CheckContext::Gentoo => repo.name() == "gentoo" || selected.contains(self),
                CheckContext::GentooInherited => repo.trees().any(|x| x.name() == "gentoo"),
                CheckContext::Optional => selected.contains(self),
                CheckContext::Overlay => !repo.masters().is_empty(),
            }
            .not()
        })
    }

    /// Determine if a check is disabled for a scanning run due to package filtering.
    pub(crate) fn filtered(&self) -> bool {
        self.scope != Scope::Version
            || (self.source != SourceKind::EbuildPkg
                && self.source != SourceKind::EbuildRawPkg)
    }

    /// Determine if a check is disabled for a scanning run due to scan scope.
    pub(crate) fn scoped(&self, scope: Scope) -> Option<Scope> {
        if self.scope > scope {
            Some(self.scope)
        } else {
            None
        }
    }

    /// Check requires post-run finalization.
    pub(crate) fn finalize(&self) -> bool {
        self.reports.iter().any(|r| r.finalize())
    }
}

/// Create a check runner from a given check.
pub(crate) trait ToRunner<T> {
    fn to_runner(&self, repo: &EbuildRepo, filter: &ReportFilter) -> T;
}

impl ToRunner<EbuildPkgRunner> for Check {
    fn to_runner(&self, repo: &EbuildRepo, filter: &ReportFilter) -> EbuildPkgRunner {
        match &self.kind {
            CheckKind::Dependency => Box::new(dependency::create(repo, filter)),
            CheckKind::DependencySlotMissing => {
                Box::new(dependency_slot_missing::create(repo))
            }
            CheckKind::Homepage => Box::new(homepage::create()),
            CheckKind::Iuse => Box::new(iuse::create(repo, filter)),
            CheckKind::Keywords => Box::new(keywords::create(repo, filter)),
            CheckKind::License => Box::new(license::create(repo, filter)),
            CheckKind::Overlay => Box::new(overlay::create(repo)),
            CheckKind::Properties => Box::new(properties::create(repo)),
            CheckKind::PythonUpdate => Box::new(python_update::create(repo)),
            CheckKind::Restrict => Box::new(restrict::create(repo)),
            CheckKind::RestrictTestMissing => Box::new(restrict_test_missing::create()),
            CheckKind::RubyUpdate => Box::new(ruby_update::create(repo)),
            CheckKind::SrcUri => Box::new(src_uri::create(repo, filter)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<EbuildPkgSetRunner> for Check {
    fn to_runner(&self, repo: &EbuildRepo, _filter: &ReportFilter) -> EbuildPkgSetRunner {
        match &self.kind {
            CheckKind::Filesdir => Box::new(filesdir::create(repo)),
            CheckKind::EapiStale => Box::new(eapi_stale::create()),
            CheckKind::KeywordsDropped => Box::new(keywords_dropped::create(repo)),
            CheckKind::Live => Box::new(live::create()),
            CheckKind::Manifest => Box::new(manifest::create(repo)),
            CheckKind::UnstableOnly => Box::new(unstable_only::create(repo)),
            CheckKind::UseLocal => Box::new(use_local::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<EbuildRawPkgRunner> for Check {
    fn to_runner(&self, repo: &EbuildRepo, _filter: &ReportFilter) -> EbuildRawPkgRunner {
        match &self.kind {
            CheckKind::Commands => Box::new(commands::create()),
            CheckKind::EapiStatus => Box::new(eapi_status::create(repo)),
            CheckKind::Header => Box::new(header::create()),
            CheckKind::VariableOrder => Box::new(variable_order::create()),
            CheckKind::Whitespace => Box::new(whitespace::create()),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<EbuildRawPkgSetRunner> for Check {
    fn to_runner(&self, _repo: &EbuildRepo, _filter: &ReportFilter) -> EbuildRawPkgSetRunner {
        unreachable!("unsupported check: {self}")
    }
}

impl ToRunner<CpnRunner> for Check {
    fn to_runner(&self, repo: &EbuildRepo, _filter: &ReportFilter) -> CpnRunner {
        match &self.kind {
            CheckKind::EbuildName => Box::new(ebuild_name::create(repo)),
            CheckKind::Duplicates => Box::new(duplicates::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<CpvRunner> for Check {
    fn to_runner(&self, repo: &EbuildRepo, _filter: &ReportFilter) -> CpvRunner {
        match &self.kind {
            CheckKind::Metadata => Box::new(metadata::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<RepoRunner> for Check {
    fn to_runner(&self, _repo: &EbuildRepo, _filter: &ReportFilter) -> RepoRunner {
        match &self.kind {
            CheckKind::RepoLayout => Box::new(repo_layout::create()),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl fmt::Debug for Check {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
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

impl AsRef<Utf8Path> for Check {
    fn as_ref(&self) -> &Utf8Path {
        Utf8Path::new(&self.kind)
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
    use itertools::Itertools;
    use pkgcraft::test::assert_ordered_eq;
    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn kind() {
        // verify CheckKind are kept in lexical order
        let kinds: Vec<_> = CheckKind::iter().collect();
        let ordered: Vec<_> = CheckKind::iter().map(|x| x.to_string()).sorted().collect();
        let ordered: Vec<_> = ordered.iter().map(|s| s.parse().unwrap()).collect();
        assert_ordered_eq!(&kinds, &ordered);

        // verify all CheckKind variants map to implemented checks
        let checks: Vec<_> = Check::iter().map(|x| x.kind).collect();
        assert_ordered_eq!(&kinds, &checks);
    }

    #[test]
    fn display_and_debug() {
        for check in Check::iter() {
            let s = check.to_string();
            assert_eq!(format!("{check:?}"), s);
        }
    }

    #[test]
    fn report() {
        // verify all report variants have at least one check
        let reports: Vec<_> = ReportKind::iter()
            .filter(|x| REPORTS.get(x).is_none())
            .collect();
        assert!(reports.is_empty(), "no checks for reports: {}", reports.iter().join(", "));
    }

    #[test]
    fn source() {
        // verify all source variants have at least one check
        let sources: Vec<_> = SourceKind::iter()
            .filter(|x| Check::iter_source(x).next().is_none())
            .collect();
        assert!(sources.is_empty(), "no checks for sources: {}", sources.iter().join(", "));
    }
}
