use std::collections::HashSet;
use std::fmt;
use std::ops::Not;
use std::sync::LazyLock;

use camino::Utf8Path;
use indexmap::IndexSet;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::ebuild::{EbuildPkg, EbuildRawPkg};
use pkgcraft::repo::{ebuild::EbuildRepo, Repository};
use pkgcraft::restrict::Scope;
use pkgcraft::types::{OrderedMap, OrderedSet};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};

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
pub enum Check {
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
    Whitespace,
}

impl Check {
    /// Return the name of a check.
    pub fn name(&self) -> &str {
        self.as_ref()
    }

    /// All the potential report variants generated by the check.
    pub fn reports(&self) -> &'static [ReportKind] {
        use ReportKind::*;
        match self {
            Self::Commands => &[Builtin, Optfeature],
            Self::Dependency => &[
                DependencyDeprecated,
                DependencyInvalid,
                DependencyRevisionMissing,
                PackageDeprecatedUnused,
            ],
            Self::DependencySlotMissing => &[DependencySlotMissing],
            Self::Duplicates => &[PackageOverride],
            Self::EapiStale => &[EapiStale],
            Self::EapiStatus => &[EapiBanned, EapiDeprecated, EapiUnused],
            Self::Filesdir => &[FileUnknown, FilesUnused],
            Self::EbuildName => &[EbuildNameInvalid, EbuildVersionsEqual],
            Self::Eclass => &[EclassUnused],
            Self::Header => &[HeaderInvalid],
            Self::Homepage => &[HomepageInvalid],
            Self::Ignore => &[IgnoreUnused],
            Self::Iuse => &[IuseInvalid, UseGlobalUnused],
            Self::Keywords => &[
                EapiUnstable,
                KeywordsLive,
                KeywordsOverlapping,
                KeywordsUnsorted,
                ArchesUnused,
            ],
            Self::KeywordsDropped => &[KeywordsDropped],
            Self::License => &[LicenseDeprecated, LicensesUnused, LicenseInvalid],
            Self::Live => &[LiveOnly],
            Self::Manifest => &[ManifestInvalid, ManifestConflict, ManifestCollide],
            Self::Metadata => &[MetadataError],
            Self::Properties => &[PropertiesInvalid],
            Self::PythonUpdate => &[PythonUpdate],
            Self::RepoLayout => &[RepoCategoryEmpty, RepoCategoriesUnused, RepoPackageEmpty],
            Self::Restrict => &[RestrictInvalid],
            Self::RestrictTestMissing => &[RestrictMissing],
            Self::RubyUpdate => &[RubyUpdate],
            Self::SrcUri => &[MirrorsUnused, UriInvalid],
            Self::UnstableOnly => &[UnstableOnly],
            Self::UseLocal => {
                &[UseLocalDescMissing, UseLocalGlobal, UseLocalUnused, UseLocalUnsorted]
            }
            Self::VariableOrder => &[VariableOrder],
            Self::Whitespace => &[EapiFormat, WhitespaceInvalid, WhitespaceUnneeded],
        }
    }

    /// The minimum scope the check can run in.
    pub(crate) fn scope(&self) -> Scope {
        match self {
            Self::Commands => Scope::Version,
            Self::Dependency => Scope::Version,
            Self::DependencySlotMissing => Scope::Version,
            Self::Duplicates => Scope::Package,
            Self::EapiStale => Scope::Package,
            Self::EapiStatus => Scope::Version,
            Self::Filesdir => Scope::Package,
            Self::EbuildName => Scope::Package,
            Self::Eclass => Scope::Version,
            Self::Header => Scope::Version,
            Self::Homepage => Scope::Version,
            Self::Ignore => Scope::Version,
            Self::Iuse => Scope::Version,
            Self::Keywords => Scope::Version,
            Self::KeywordsDropped => Scope::Package,
            Self::License => Scope::Version,
            Self::Live => Scope::Package,
            Self::Manifest => Scope::Package,
            Self::Metadata => Scope::Version,
            Self::Properties => Scope::Version,
            Self::PythonUpdate => Scope::Version,
            Self::RepoLayout => Scope::Repo,
            Self::Restrict => Scope::Version,
            Self::RestrictTestMissing => Scope::Version,
            Self::RubyUpdate => Scope::Version,
            Self::SrcUri => Scope::Version,
            Self::UnstableOnly => Scope::Package,
            Self::UseLocal => Scope::Package,
            Self::VariableOrder => Scope::Version,
            Self::Whitespace => Scope::Version,
        }
    }

    /// The sources of values a check can run against.
    pub(crate) fn sources(&self) -> &[SourceKind] {
        match self {
            Self::Commands => &[SourceKind::EbuildRawPkg],
            Self::Dependency => &[SourceKind::EbuildPkg],
            Self::DependencySlotMissing => &[SourceKind::EbuildPkg],
            Self::Duplicates => &[SourceKind::Cpn],
            Self::EapiStale => &[SourceKind::EbuildPkg],
            Self::EapiStatus => &[SourceKind::EbuildRawPkg],
            Self::Filesdir => &[SourceKind::EbuildPkg],
            Self::EbuildName => &[SourceKind::Cpn],
            Self::Eclass => &[SourceKind::EbuildPkg],
            Self::Header => &[SourceKind::EbuildRawPkg],
            Self::Homepage => &[SourceKind::EbuildPkg],
            Self::Ignore => &[SourceKind::Cpv, SourceKind::Cpn, SourceKind::Repo],
            Self::Iuse => &[SourceKind::EbuildPkg],
            Self::Keywords => &[SourceKind::EbuildPkg],
            Self::KeywordsDropped => &[SourceKind::EbuildPkg],
            Self::License => &[SourceKind::EbuildPkg],
            Self::Live => &[SourceKind::EbuildPkg],
            Self::Manifest => &[SourceKind::EbuildPkg],
            Self::Metadata => &[SourceKind::Cpv],
            Self::Properties => &[SourceKind::EbuildPkg],
            Self::PythonUpdate => &[SourceKind::EbuildPkg],
            Self::RepoLayout => &[SourceKind::Repo],
            Self::Restrict => &[SourceKind::EbuildPkg],
            Self::RestrictTestMissing => &[SourceKind::EbuildPkg],
            Self::RubyUpdate => &[SourceKind::EbuildPkg],
            Self::SrcUri => &[SourceKind::EbuildPkg],
            Self::UnstableOnly => &[SourceKind::EbuildPkg],
            Self::UseLocal => &[SourceKind::EbuildPkg],
            Self::VariableOrder => &[SourceKind::EbuildRawPkg],
            Self::Whitespace => &[SourceKind::EbuildRawPkg],
        }
    }

    /// Contexts required to run the check.
    pub fn context(&self) -> &[CheckContext] {
        use CheckContext::*;
        match self {
            Self::Duplicates => &[Optional, Overlay],
            Self::Header => &[Gentoo],
            Self::Ignore => &[Optional],
            Self::Live => &[Gentoo],
            Self::PythonUpdate => &[GentooInherited],
            Self::RubyUpdate => &[GentooInherited],
            Self::UnstableOnly => &[Optional],
            _ => &[],
        }
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
        Self::iter()
            .filter(move |x| x.skipped(repo, &selected).is_none() && scope >= x.scope())
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
        Self::iter().filter(move |c| c.sources().contains(source))
    }

    /// Determine if a check is skipped for a scanning run due to scan context.
    pub(crate) fn skipped(
        &self,
        repo: &EbuildRepo,
        selected: &IndexSet<Self>,
    ) -> Option<CheckContext> {
        self.context().iter().copied().find(|context| {
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
        self.scope() != Scope::Version
            || (!self.sources().contains(&SourceKind::EbuildPkg)
                && !self.sources().contains(&SourceKind::EbuildRawPkg))
    }

    /// Determine if a check is disabled for a scanning run due to scan scope.
    pub(crate) fn scoped(&self, scope: Scope) -> Option<Scope> {
        if self.scope() > scope {
            Some(self.scope())
        } else {
            None
        }
    }

    /// Check requires post-run finalization for a scope.
    pub(crate) fn finalize(&self, enabled: &HashSet<ReportKind>) -> bool {
        self.reports()
            .iter()
            .any(|r| r.finalize() && enabled.contains(r))
    }
}

impl AsRef<Utf8Path> for Check {
    fn as_ref(&self) -> &Utf8Path {
        Utf8Path::new(self.name())
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
    fn run(&self, repo: &EbuildRepo, filter: &ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type RepoRunner = Box<dyn RepoCheck + Send + Sync>;

/// Run a check against a Cpv.
pub(crate) trait CpvCheck: fmt::Display {
    fn run(&self, cpv: &Cpv, filter: &ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type CpvRunner = Box<dyn CpvCheck + Send + Sync>;

/// Run a check against a Cpn.
pub(crate) trait CpnCheck: fmt::Display {
    fn run(&self, cpn: &Cpn, filter: &ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type CpnRunner = Box<dyn CpnCheck + Send + Sync>;

/// Run a check against a given ebuild package version.
pub(crate) trait EbuildPkgCheck: fmt::Display {
    fn run(&self, pkg: &EbuildPkg, filter: &ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type EbuildPkgRunner = Box<dyn EbuildPkgCheck + Send + Sync>;

/// Run a check against a given ebuild package set.
pub(crate) trait EbuildPkgSetCheck: fmt::Display {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildPkg], filter: &ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type EbuildPkgSetRunner = Box<dyn EbuildPkgSetCheck + Send + Sync>;

/// Run a check against a given raw ebuild package version.
pub(crate) trait EbuildRawPkgCheck: fmt::Display {
    fn run(&self, pkg: &EbuildRawPkg, filter: &ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type EbuildRawPkgRunner = Box<dyn EbuildRawPkgCheck + Send + Sync>;

/// Run a check against a raw ebuild package set.
pub(crate) trait EbuildRawPkgSetCheck: fmt::Display {
    fn run(&self, cpn: &Cpn, pkgs: &[EbuildRawPkg], filter: &ReportFilter);
    fn finish(&self, _repo: &EbuildRepo, _filter: &ReportFilter) {
        unimplemented!("{self} finish")
    }
}
pub(crate) type EbuildRawPkgSetRunner = Box<dyn EbuildRawPkgSetCheck + Send + Sync>;

/// Create a check runner from a given check.
pub(crate) trait ToRunner<T> {
    fn to_runner(&self, repo: &EbuildRepo, filter: &ReportFilter) -> T;
}

impl ToRunner<EbuildPkgRunner> for Check {
    fn to_runner(&self, repo: &EbuildRepo, filter: &ReportFilter) -> EbuildPkgRunner {
        match self {
            Self::Dependency => Box::new(dependency::create(repo, filter)),
            Self::DependencySlotMissing => Box::new(dependency_slot_missing::create(repo)),
            Self::Eclass => Box::new(eclass::create(repo, filter)),
            Self::Homepage => Box::new(homepage::create()),
            Self::Iuse => Box::new(iuse::create(repo, filter)),
            Self::Keywords => Box::new(keywords::create(repo, filter)),
            Self::License => Box::new(license::create(repo, filter)),
            Self::Properties => Box::new(properties::create(repo)),
            Self::PythonUpdate => Box::new(python_update::create(repo)),
            Self::Restrict => Box::new(restrict::create(repo)),
            Self::RestrictTestMissing => Box::new(restrict_test_missing::create()),
            Self::RubyUpdate => Box::new(ruby_update::create(repo)),
            Self::SrcUri => Box::new(src_uri::create(repo, filter)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<EbuildPkgSetRunner> for Check {
    fn to_runner(&self, repo: &EbuildRepo, _filter: &ReportFilter) -> EbuildPkgSetRunner {
        match self {
            Self::Filesdir => Box::new(filesdir::create(repo)),
            Self::EapiStale => Box::new(eapi_stale::create()),
            Self::KeywordsDropped => Box::new(keywords_dropped::create(repo)),
            Self::Live => Box::new(live::create()),
            Self::Manifest => Box::new(manifest::create(repo)),
            Self::UnstableOnly => Box::new(unstable_only::create(repo)),
            Self::UseLocal => Box::new(use_local::create(repo)),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<EbuildRawPkgRunner> for Check {
    fn to_runner(&self, repo: &EbuildRepo, filter: &ReportFilter) -> EbuildRawPkgRunner {
        match self {
            Self::Commands => Box::new(commands::create()),
            Self::EapiStatus => Box::new(eapi_status::create(repo, filter)),
            Self::Header => Box::new(header::create()),
            Self::VariableOrder => Box::new(variable_order::create()),
            Self::Whitespace => Box::new(whitespace::create()),
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
        match self {
            Self::EbuildName => Box::new(ebuild_name::create(repo)),
            Self::Duplicates => Box::new(duplicates::create(repo)),
            Self::Ignore => Box::new(ignore::Check),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<CpvRunner> for Check {
    fn to_runner(&self, repo: &EbuildRepo, _filter: &ReportFilter) -> CpvRunner {
        match self {
            Self::Metadata => Box::new(metadata::create(repo)),
            Self::Ignore => Box::new(ignore::Check),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

impl ToRunner<RepoRunner> for Check {
    fn to_runner(&self, _repo: &EbuildRepo, _filter: &ReportFilter) -> RepoRunner {
        match self {
            Self::Ignore => Box::new(ignore::Check),
            Self::RepoLayout => Box::new(repo_layout::create()),
            _ => unreachable!("unsupported check: {self}"),
        }
    }
}

/// The mapping of all report variants to the checks that can generate them.
static REPORTS: LazyLock<OrderedMap<ReportKind, OrderedSet<Check>>> = LazyLock::new(|| {
    Check::iter()
        .flat_map(|c| c.reports().iter().copied().map(move |r| (r, c)))
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
        // verify checks are registered in lexical order
        let kinds: Vec<_> = Check::iter().collect();
        let ordered: Vec<_> = Check::iter().map(|x| x.to_string()).sorted().collect();
        let ordered: Vec<_> = ordered.iter().map(|s| s.parse().unwrap()).collect();
        assert_ordered_eq!(&kinds, &ordered);
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
