use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::LazyLock;

use camino::Utf8Path;
use indexmap::IndexSet;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::ebuild::EbuildPkg;
use pkgcraft::repo::{ebuild::EbuildRepo, Repository};
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};

use crate::report::ReportKind;
use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::source::{EbuildParsedPkg, SourceKind};
use crate::Error;

mod builtins;
mod dependency;
mod dependency_slot_missing;
mod duplicates;
mod eapi_stale;
mod eapi_status;
mod ebuild_files;
mod ebuild_name;
mod header;
mod keywords;
mod keywords_dropped;
mod license;
mod live;
mod metadata;
mod overlay;
mod properties;
mod python_update;
mod repo_layout;
mod restrict;
mod restrict_test_missing;
mod ruby_update;
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
    Builtins,
    Dependency,
    DependencySlotMissing,
    Duplicates,
    EapiStale,
    EapiStatus,
    EbuildFiles,
    EbuildName,
    Header,
    Keywords,
    KeywordsDropped,
    License,
    Live,
    Metadata,
    Overlay,
    Properties,
    PythonUpdate,
    RepoLayout,
    Restrict,
    RestrictTestMissing,
    RubyUpdate,
    UnstableOnly,
    UseLocal,
    VariableOrder,
    Whitespace,
}

impl From<CheckKind> for Check {
    fn from(value: CheckKind) -> Self {
        use CheckKind::*;
        match value {
            Builtins => builtins::CHECK,
            Dependency => dependency::CHECK,
            DependencySlotMissing => dependency_slot_missing::CHECK,
            Duplicates => duplicates::CHECK,
            EapiStale => eapi_stale::CHECK,
            EapiStatus => eapi_status::CHECK,
            EbuildFiles => ebuild_files::CHECK,
            EbuildName => ebuild_name::CHECK,
            Header => header::CHECK,
            Keywords => keywords::CHECK,
            KeywordsDropped => keywords_dropped::CHECK,
            License => license::CHECK,
            Live => live::CHECK,
            Overlay => overlay::CHECK,
            Metadata => metadata::CHECK,
            Properties => properties::CHECK,
            PythonUpdate => python_update::CHECK,
            RepoLayout => repo_layout::CHECK,
            Restrict => restrict::CHECK,
            RestrictTestMissing => restrict_test_missing::CHECK,
            RubyUpdate => ruby_update::CHECK,
            UnstableOnly => unstable_only::CHECK,
            UseLocal => use_local::CHECK,
            VariableOrder => variable_order::CHECK,
            Whitespace => whitespace::CHECK,
        }
    }
}

/// Check contexts.
#[derive(PartialEq, Eq, Hash, Copy, Clone)]
enum CheckContext {
    /// Check only runs by default in the gentoo repo.
    Gentoo,

    /// Check only runs in repos inheriting from the gentoo repo.
    GentooInherited,

    /// Check isn't enabled by default.
    Optional,

    /// Check only runs in overlay repos.
    Overlay,
}

/// Run a check against a repo.
pub(crate) trait RepoCheck {
    fn run(&self, repo: &EbuildRepo, filter: &mut ReportFilter);
}
pub(crate) type RepoRunner = Box<dyn RepoCheck + Send + Sync>;

/// Run a check against a Cpv.
pub(crate) trait CpvCheck {
    fn run(&self, cpv: &Cpv, filter: &mut ReportFilter);
}
pub(crate) type CpvRunner = Box<dyn CpvCheck + Send + Sync>;

/// Run a check against a Cpn.
pub(crate) trait CpnCheck {
    fn run(&self, cpn: &Cpn, filter: &mut ReportFilter);
}
pub(crate) type CpnRunner = Box<dyn CpnCheck + Send + Sync>;

/// Run a check against a given ebuild package version.
pub(crate) trait EbuildPkgCheck {
    fn run(&self, pkg: &EbuildPkg, filter: &mut ReportFilter);
}
pub(crate) type EbuildPkgRunner = Box<dyn EbuildPkgCheck + Send + Sync>;

/// Run a check against a given ebuild package set.
pub(crate) trait EbuildPkgSetCheck {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &mut ReportFilter);
}
pub(crate) type EbuildPkgSetRunner = Box<dyn EbuildPkgSetCheck + Send + Sync>;

/// Run a check against a given raw ebuild package version.
pub(crate) trait EbuildRawPkgCheck {
    fn run(&self, pkg: &EbuildParsedPkg, filter: &mut ReportFilter);
}
pub(crate) type EbuildRawPkgRunner = Box<dyn EbuildRawPkgCheck + Send + Sync>;

/// Run a check against a raw ebuild package set.
pub(crate) trait EbuildRawPkgSetCheck {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildParsedPkg], filter: &mut ReportFilter);
}
pub(crate) type EbuildRawPkgSetRunner = Box<dyn EbuildRawPkgSetCheck + Send + Sync>;

/// Registered check.
#[derive(Copy, Clone)]
pub struct Check {
    /// The check variant.
    pub(crate) kind: CheckKind,

    /// The scope the check runs in.
    pub scope: Scope,

    /// The source of the values the check runs against.
    pub source: SourceKind,

    /// All the potential report variants generated by the check.
    pub reports: &'static [ReportKind],

    /// Check variant contexts.
    context: &'static [CheckContext],

    /// The priority of the check for enabling a deterministic running order.
    priority: i64,
}

impl Check {
    /// Return the name of the check.
    pub fn name(&self) -> &str {
        self.kind.as_ref()
    }

    /// Return an iterator of all registered checks.
    pub fn iter() -> impl Iterator<Item = Check> {
        CheckKind::iter().map(Into::into)
    }

    /// Return an iterator of all checks enabled by default.
    pub fn iter_default(target_repo: Option<&EbuildRepo>) -> Box<dyn Iterator<Item = Check> + '_> {
        let selected = IndexSet::new();
        if let Some(repo) = target_repo {
            Box::new(Check::iter().filter(move |x| x.enabled(repo, &selected)))
        } else {
            Box::new(Check::iter().filter(|x| !x.context.contains(&CheckContext::Optional)))
        }
    }

    /// Return an iterator of checks that generate a given report.
    pub fn iter_report(report: &ReportKind) -> impl Iterator<Item = Check> {
        REPORT_CHECKS
            .get(report)
            .unwrap_or_else(|| unreachable!("no checks for report: {report}"))
            .iter()
            .copied()
    }

    /// Return an iterator of checks that use a given source.
    pub fn iter_source(source: &SourceKind) -> impl Iterator<Item = Check> {
        SOURCE_CHECKS
            .get(source)
            .unwrap_or_else(|| unreachable!("no checks for source: {source}"))
            .iter()
            .copied()
    }

    /// Determine if a check is enabled for a scanning run due to scan context.
    pub(crate) fn enabled(&self, repo: &EbuildRepo, selected: &IndexSet<Self>) -> bool {
        self.context.iter().all(|x| match x {
            CheckContext::Gentoo => repo.name() == "gentoo" || selected.contains(self),
            CheckContext::GentooInherited => repo.trees().any(|x| x.name() == "gentoo"),
            CheckContext::Optional => selected.contains(self),
            CheckContext::Overlay => !repo.masters().is_empty(),
        })
    }

    /// Determine if a check is disabled for a scanning run due to package filtering.
    pub(crate) fn filtered(&self) -> bool {
        self.scope != Scope::Version
            || (self.source != SourceKind::EbuildPkg && self.source != SourceKind::EbuildRawPkg)
    }
}

/// Create a check runner from a given check.
pub(crate) trait ToRunner<T> {
    fn to_runner(&self, repo: &'static EbuildRepo) -> T;
}

impl ToRunner<EbuildPkgRunner> for Check {
    fn to_runner(&self, repo: &'static EbuildRepo) -> EbuildPkgRunner {
        match &self.kind {
            CheckKind::Dependency => Box::new(dependency::create(repo)),
            CheckKind::DependencySlotMissing => Box::new(dependency_slot_missing::create(repo)),
            CheckKind::Keywords => Box::new(keywords::create(repo)),
            CheckKind::License => Box::new(license::create(repo)),
            CheckKind::Overlay => Box::new(overlay::create(repo)),
            CheckKind::Properties => Box::new(properties::create(repo)),
            CheckKind::PythonUpdate => Box::new(python_update::create(repo)),
            CheckKind::Restrict => Box::new(restrict::create(repo)),
            CheckKind::RestrictTestMissing => Box::new(restrict_test_missing::create()),
            CheckKind::RubyUpdate => Box::new(ruby_update::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<EbuildPkgSetRunner> for Check {
    fn to_runner(&self, repo: &'static EbuildRepo) -> EbuildPkgSetRunner {
        match &self.kind {
            CheckKind::EapiStale => Box::new(eapi_stale::create()),
            CheckKind::KeywordsDropped => Box::new(keywords_dropped::create(repo)),
            CheckKind::Live => Box::new(live::create()),
            CheckKind::UnstableOnly => Box::new(unstable_only::create(repo)),
            CheckKind::UseLocal => Box::new(use_local::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<EbuildRawPkgRunner> for Check {
    fn to_runner(&self, repo: &'static EbuildRepo) -> EbuildRawPkgRunner {
        match &self.kind {
            CheckKind::Builtins => Box::new(builtins::create()),
            CheckKind::EapiStatus => Box::new(eapi_status::create(repo)),
            CheckKind::Header => Box::new(header::create()),
            CheckKind::VariableOrder => Box::new(variable_order::create()),
            CheckKind::Whitespace => Box::new(whitespace::create()),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<EbuildRawPkgSetRunner> for Check {
    fn to_runner(&self, repo: &'static EbuildRepo) -> EbuildRawPkgSetRunner {
        match &self.kind {
            CheckKind::EbuildFiles => Box::new(ebuild_files::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<CpnRunner> for Check {
    fn to_runner(&self, repo: &'static EbuildRepo) -> CpnRunner {
        match &self.kind {
            CheckKind::EbuildName => Box::new(ebuild_name::create(repo)),
            CheckKind::Duplicates => Box::new(duplicates::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<CpvRunner> for Check {
    fn to_runner(&self, repo: &'static EbuildRepo) -> CpvRunner {
        match &self.kind {
            CheckKind::Metadata => Box::new(metadata::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<RepoRunner> for Check {
    fn to_runner(&self, _repo: &'static EbuildRepo) -> RepoRunner {
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

impl FromStr for Check {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let kind: CheckKind = s
            .parse()
            .map_err(|_| Error::InvalidValue(format!("unknown check: {s}")))?;

        Ok(kind.into())
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
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.kind.cmp(&other.kind))
    }
}

impl PartialOrd for Check {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl AsRef<Utf8Path> for Check {
    fn as_ref(&self) -> &Utf8Path {
        Utf8Path::new(self.name())
    }
}

/// The mapping of all report variants to the checks that can generate them.
static REPORT_CHECKS: LazyLock<OrderedMap<ReportKind, OrderedSet<Check>>> = LazyLock::new(|| {
    Check::iter()
        .flat_map(|c| c.reports.iter().copied().map(move |r| (r, c)))
        .collect()
});

/// The mapping of all source variants to the checks that use them.
static SOURCE_CHECKS: LazyLock<OrderedMap<SourceKind, OrderedSet<Check>>> =
    LazyLock::new(|| Check::iter().map(|c| (c.source, c)).collect());

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
    fn report() {
        // verify all report variants have at least one check
        let reports: Vec<_> = ReportKind::iter()
            .filter(|x| REPORT_CHECKS.get(x).is_none())
            .collect();
        assert!(reports.is_empty(), "no checks for reports: {}", reports.iter().join(", "));
    }

    #[test]
    fn source() {
        // verify all source variants have at least one check
        let sources: Vec<_> = SourceKind::iter()
            .filter(|x| SOURCE_CHECKS.get(x).is_none())
            .collect();
        assert!(sources.is_empty(), "no checks for sources: {}", sources.iter().join(", "));
    }
}
