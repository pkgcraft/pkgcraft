use std::cmp::Ordering;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;

use camino::Utf8Path;
use colored::Color;
use indexmap::IndexSet;
use pkgcraft::bash::Node;
use pkgcraft::cli::TriState;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::repo::{ebuild::EbuildRepo, Repository};
use pkgcraft::restrict::{Restrict, Restriction, Scope};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString};

use crate::check::{Check, CheckContext};
use crate::iter::ReportFilter;
use crate::Error;

/// The severity of the report.
#[derive(
    Display, EnumIter, EnumString, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
#[strum(serialize_all = "kebab-case")]
pub enum ReportLevel {
    Critical,
    Error,
    Warning,
    Style,
    Info,
}

impl From<ReportLevel> for Color {
    fn from(level: ReportLevel) -> Self {
        match level {
            ReportLevel::Critical => Color::Red,
            ReportLevel::Error => Color::TrueColor { r: 255, g: 140, b: 0 },
            ReportLevel::Warning => Color::Yellow,
            ReportLevel::Style => Color::Cyan,
            ReportLevel::Info => Color::Green,
        }
    }
}

/// Report sets that relate to one or more variants.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum ReportSet {
    All,
    Finalize,
    Check(Check),
    Context(CheckContext),
    Level(ReportLevel),
    Report(ReportKind),
    Scope(Scope),
}

impl From<Check> for ReportSet {
    fn from(value: Check) -> Self {
        Self::Check(value)
    }
}

impl From<CheckContext> for ReportSet {
    fn from(value: CheckContext) -> Self {
        Self::Context(value)
    }
}

impl From<ReportLevel> for ReportSet {
    fn from(value: ReportLevel) -> Self {
        Self::Level(value)
    }
}

impl From<ReportKind> for ReportSet {
    fn from(value: ReportKind) -> Self {
        Self::Report(value)
    }
}

impl From<Scope> for ReportSet {
    fn from(value: Scope) -> Self {
        Self::Scope(value)
    }
}

impl FromStr for ReportSet {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(val) = s.strip_prefix('@') {
            match val {
                "all" => Ok(Self::All),
                "finalize" => Ok(Self::Finalize),
                _ => val
                    .parse()
                    .map(Self::Check)
                    .or_else(|_| val.parse().map(Self::Context))
                    .or_else(|_| val.parse().map(Self::Level))
                    .or_else(|_| val.parse().map(Self::Scope))
                    .map_err(|_| Error::InvalidValue(format!("invalid report set: {val}"))),
            }
        } else {
            s.parse()
                .map(Self::Report)
                .map_err(|_| Error::InvalidValue(format!("invalid report: {s}")))
        }
    }
}

impl fmt::Display for ReportSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::All => write!(f, "@all"),
            Self::Finalize => write!(f, "@finalize"),
            Self::Check(check) => write!(f, "@{check}"),
            Self::Context(context) => write!(f, "@{context}"),
            Self::Level(level) => write!(f, "@{level}"),
            Self::Report(report) => write!(f, "{report}"),
            Self::Scope(scope) => write!(f, "@{scope}"),
        }
    }
}

impl ReportSet {
    /// Return true if the related reports should be added to the selected set.
    pub fn selected(&self) -> bool {
        matches!(self, Self::Report(_) | Self::Check(_))
    }

    /// Expand a report set into an iterator of its variants.
    pub fn expand<'a>(
        self,
        default: &'a IndexSet<ReportKind>,
        supported: &'a IndexSet<ReportKind>,
    ) -> Box<dyn Iterator<Item = ReportKind> + 'a> {
        match self {
            Self::All => Box::new(supported.iter().copied()),
            Self::Finalize => Box::new(
                default
                    .iter()
                    .filter(|r| r.finish_check(Scope::Repo))
                    .copied(),
            ),
            Self::Check(check) => Box::new(check.reports().iter().copied()),
            Self::Context(context) => Box::new(
                Check::iter_report(supported)
                    .filter(move |x| x.context().contains(&context))
                    .flat_map(|x| x.reports())
                    .copied(),
            ),
            Self::Level(level) => {
                Box::new(default.iter().filter(move |r| r.level() == level).copied())
            }
            Self::Report(kind) => Box::new([kind].into_iter()),
            Self::Scope(scope) => {
                Box::new(default.iter().filter(move |r| r.scope() == scope).copied())
            }
        }
    }
}

/// Wrapper for report set targets.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub struct ReportTarget(TriState<ReportSet>);

impl ReportTarget {
    /// Collapse report targets into default and selected report variant sets.
    pub fn collapse<'a, I>(
        values: I,
        defaults: &IndexSet<ReportKind>,
        supported: &IndexSet<ReportKind>,
    ) -> crate::Result<(IndexSet<ReportKind>, IndexSet<ReportKind>)>
    where
        I: IntoIterator<Item = &'a Self>,
    {
        // sort sets by variant
        let mut values: IndexSet<_> = values.into_iter().copied().map(|x| x.0).collect();
        values.sort();

        // don't use defaults if neutral options exist
        let mut enabled = if let Some(TriState::Set(_)) = values.first() {
            Default::default()
        } else {
            defaults.clone()
        };

        // Expand report sets, only adding explicitly selected check and report variants
        // to the selection set. Set membership determines if an enabled check is skipped
        // with a warning or errors out if it is unable to be run.
        let mut selected = IndexSet::new();
        for x in values {
            match x {
                TriState::Set(set) | TriState::Add(set) => {
                    for r in set.expand(defaults, supported) {
                        enabled.insert(r);
                        // track explicitly selected or supported variants
                        if set.selected() || supported.contains(&r) {
                            selected.insert(r);
                        }
                    }
                }
                TriState::Remove(set) => {
                    for r in set.expand(defaults, supported) {
                        enabled.swap_remove(&r);
                    }
                }
            };
        }

        if enabled.is_empty() {
            Err(Error::InvalidValue("no reports enabled".to_string()))
        } else {
            enabled.sort();
            selected.sort();
            Ok((enabled, selected))
        }
    }
}

impl<T: Into<ReportSet>> From<T> for ReportTarget {
    fn from(value: T) -> Self {
        Self(TriState::Set(value.into()))
    }
}

impl FromStr for ReportTarget {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

/// Report variants.
#[derive(
    Serialize,
    Deserialize,
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
pub enum ReportKind {
    /// Arches that are unused.
    ArchesUnused,

    /// Ebuild uses a bash builtin as an external command.
    Builtin,

    /// Package dependency flagged as deprecated by the repo.
    DependencyDeprecated,

    /// Ebuild has an invalid dependency.
    DependencyInvalid,

    /// Package dependency is missing a revision.
    DependencyRevisionMissing,

    /// Dependency is missing a slot.
    DependencySlotMissing,

    /// Package has a banned EAPI.
    EapiBanned,

    /// Package has a deprecated EAPI.
    EapiDeprecated,

    /// Ebuild has a non-standard EAPI assignment format.
    ///
    /// The EAPI assignment should be wrapped in empty lines (except when the first line
    /// of the ebuild) with no whitespace prefix.
    EapiFormat,

    /// Package has an older EAPI than the previous release in the same SLOT.
    EapiStale,

    /// Package has stable keywords with an unstable EAPI.
    EapiUnstable,

    /// EAPIs that are unused by ebuilds in the repo.
    EapiUnused,

    /// Ebuild file has a mismatched package name or invalid version.
    EbuildNameInvalid,

    /// Multiple ebuild versions for a package are equivalent, e.g. 0 and 0-r0.
    EbuildVersionsEqual,

    /// Eclass that is unused in the parent repository.
    EclassUnused,

    /// Usage of a nonexistent file in $FILESDIR.
    FileUnknown,

    /// Package has unused files in $FILESDIR.
    FilesUnused,

    /// File has an invalid copyright and/or license header.
    HeaderInvalid,

    /// Ebuild has an invalid homepage.
    HomepageInvalid,

    /// Repo has unused pkgcruft ignore directives.
    IgnoreUnused,

    /// Ebuild has an invalid USE flag.
    IuseInvalid,

    /// Keywords have been dropped between releases.
    KeywordsDropped,

    /// Live ebuild has keywords.
    KeywordsLive,

    /// Ebuild has overlapping keywords.
    KeywordsOverlapping,

    /// Ebuild has unsorted keywords.
    KeywordsUnsorted,

    /// Ebuild has a deprecated license.
    LicenseDeprecated,

    /// Ebuild has an invalid license.
    LicenseInvalid,

    /// Repo has unused licenses.
    LicensesUnused,

    /// Package only has live ebuilds.
    LiveOnly,

    /// Package manifest has a matching hash with a different file name.
    ManifestCollide,

    /// Package manifest has matching file name with different hash.
    ManifestConflict,

    /// Package manifest is invalid.
    ManifestInvalid,

    /// Ebuild fails during metadata generation.
    MetadataError,

    /// Repo has unused mirrors.
    MirrorsUnused,

    /// Ebuild has an issue with optfeature usage.
    ///
    /// See the optfeature eclass for usage examples.
    Optfeature,

    /// Repo has unused profiles/package.deprecated entry.
    PackageDeprecatedUnused,

    /// Overlay package matches the name of a package from a parent repo.
    PackageOverride,

    /// Ebuild has invalid PROPERTIES.
    PropertiesInvalid,

    /// Ebuild can support newer python version(s).
    PythonUpdate,

    /// Repo has unused profiles/categories entry.
    RepoCategoriesUnused,

    /// Empty category directory in a repository.
    RepoCategoryEmpty,

    /// Empty package directory in a repository.
    RepoPackageEmpty,

    /// Ebuild has invalid RESTRICT.
    RestrictInvalid,

    /// Ebuild is missing a RESTRICT value of the specified type.
    RestrictMissing,

    /// Ebuild can support newer ruby version(s).
    RubyUpdate,

    /// Package only has unstable keywords.
    UnstableOnly,

    /// Ebuild has an unsupported or invalid URI.
    UriInvalid,

    /// Global USE flags that are unused.
    UseGlobalUnused,

    /// Local USE flag missing description.
    UseLocalDescMissing,

    /// Local USE flag description matching a global USE flag.
    UseLocalGlobal,

    /// Local USE flag that is unsorted.
    UseLocalUnsorted,

    /// Local USE flag that is unused.
    UseLocalUnused,

    /// Global metadata variables are defined in non-standard order.
    ///
    /// Note that this only is reported for ebuilds with all target variables
    /// unconditionally defined in global scope.
    VariableOrder,

    /// Whitespace usage that is invalid.
    WhitespaceInvalid,

    /// Whitespace usage that isn't needed.
    WhitespaceUnneeded,
}

impl ReportKind {
    /// Create a version scope report.
    pub(crate) fn version<T: Into<Cpv>>(self, value: T) -> ReportBuilder {
        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Version(value.into(), None),
            message: Default::default(),
        })
    }

    /// Create a package scope report.
    pub(crate) fn package<T>(self, value: T) -> ReportBuilder
    where
        T: TryInto<Cpn>,
        <T as TryInto<Cpn>>::Error: fmt::Display,
    {
        let cpn = value
            .try_into()
            .unwrap_or_else(|e| unreachable!("can't convert value to Cpn: {e}"));

        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Package(cpn),
            message: Default::default(),
        })
    }

    /// Create a category scope report.
    pub(crate) fn category<S: fmt::Display>(self, value: S) -> ReportBuilder {
        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Category(value.to_string()),
            message: Default::default(),
        })
    }

    /// Create a repo scope report.
    pub(crate) fn repo<R: Repository>(self, repo: R) -> ReportBuilder {
        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Repo(repo.name().to_string()),
            message: Default::default(),
        })
    }

    /// Return the severity level of the report variant.
    pub fn level(&self) -> ReportLevel {
        use ReportLevel::*;
        match self {
            Self::ArchesUnused => Warning,
            Self::Builtin => Error,
            Self::DependencyDeprecated => Warning,
            Self::DependencyInvalid => Error,
            Self::DependencyRevisionMissing => Warning,
            Self::DependencySlotMissing => Warning,
            Self::EapiBanned => Error,
            Self::EapiDeprecated => Warning,
            Self::EapiFormat => Style,
            Self::EapiStale => Warning,
            Self::EapiUnstable => Error,
            Self::EapiUnused => Warning,
            Self::EbuildNameInvalid => Error,
            Self::EbuildVersionsEqual => Error,
            Self::EclassUnused => Warning,
            Self::FileUnknown => Error,
            Self::FilesUnused => Warning,
            Self::HeaderInvalid => Error,
            Self::HomepageInvalid => Error,
            Self::IgnoreUnused => Warning,
            Self::IuseInvalid => Error,
            Self::KeywordsDropped => Warning,
            Self::KeywordsLive => Warning,
            Self::KeywordsOverlapping => Error,
            Self::KeywordsUnsorted => Style,
            Self::LicenseDeprecated => Warning,
            Self::LicenseInvalid => Error,
            Self::LicensesUnused => Warning,
            Self::LiveOnly => Warning,
            Self::ManifestInvalid => Error,
            Self::ManifestCollide => Warning,
            Self::ManifestConflict => Error,
            Self::MetadataError => Critical,
            Self::MirrorsUnused => Warning,
            Self::Optfeature => Warning,
            Self::PackageDeprecatedUnused => Warning,
            Self::PackageOverride => Warning,
            Self::PropertiesInvalid => Error,
            Self::PythonUpdate => Info,
            Self::RepoCategoriesUnused => Warning,
            Self::RepoCategoryEmpty => Warning,
            Self::RepoPackageEmpty => Warning,
            Self::RestrictInvalid => Error,
            Self::RestrictMissing => Warning,
            Self::RubyUpdate => Info,
            Self::UnstableOnly => Info,
            Self::UriInvalid => Error,
            Self::UseGlobalUnused => Warning,
            Self::UseLocalDescMissing => Error,
            Self::UseLocalGlobal => Warning,
            Self::UseLocalUnsorted => Style,
            Self::UseLocalUnused => Warning,
            Self::VariableOrder => Style,
            Self::WhitespaceInvalid => Warning,
            Self::WhitespaceUnneeded => Style,
        }
    }

    /// Return the scope of the report variant.
    ///
    /// This is the minimum scope at which the report is handled. For example, a variant
    /// with package scope isn't processed when targeting a single ebuild version.
    pub(crate) fn scope(&self) -> Scope {
        match self {
            Self::ArchesUnused => Scope::Repo,
            Self::Builtin => Scope::Version,
            Self::DependencyDeprecated => Scope::Version,
            Self::DependencyInvalid => Scope::Version,
            Self::DependencyRevisionMissing => Scope::Version,
            Self::DependencySlotMissing => Scope::Version,
            Self::EapiBanned => Scope::Version,
            Self::EapiDeprecated => Scope::Version,
            Self::EapiFormat => Scope::Version,
            Self::EapiStale => Scope::Version,
            Self::EapiUnstable => Scope::Version,
            Self::EapiUnused => Scope::Repo,
            Self::EbuildNameInvalid => Scope::Package,
            Self::EbuildVersionsEqual => Scope::Package,
            Self::EclassUnused => Scope::Repo,
            Self::FileUnknown => Scope::Version,
            Self::FilesUnused => Scope::Package,
            Self::HeaderInvalid => Scope::Version,
            Self::HomepageInvalid => Scope::Version,
            Self::IgnoreUnused => Scope::Version,
            Self::IuseInvalid => Scope::Version,
            Self::KeywordsDropped => Scope::Version,
            Self::KeywordsLive => Scope::Version,
            Self::KeywordsOverlapping => Scope::Version,
            Self::KeywordsUnsorted => Scope::Version,
            Self::LicenseDeprecated => Scope::Version,
            Self::LicenseInvalid => Scope::Version,
            Self::LicensesUnused => Scope::Repo,
            Self::LiveOnly => Scope::Package,
            Self::ManifestInvalid => Scope::Package,
            Self::ManifestCollide => Scope::Package,
            Self::ManifestConflict => Scope::Category,
            Self::MetadataError => Scope::Version,
            Self::MirrorsUnused => Scope::Repo,
            Self::Optfeature => Scope::Version,
            Self::PackageDeprecatedUnused => Scope::Repo,
            Self::PackageOverride => Scope::Package,
            Self::PropertiesInvalid => Scope::Version,
            Self::PythonUpdate => Scope::Version,
            Self::RepoCategoriesUnused => Scope::Repo,
            Self::RepoCategoryEmpty => Scope::Repo,
            Self::RepoPackageEmpty => Scope::Repo,
            Self::RestrictInvalid => Scope::Version,
            Self::RestrictMissing => Scope::Version,
            Self::RubyUpdate => Scope::Version,
            Self::UnstableOnly => Scope::Package,
            Self::UriInvalid => Scope::Version,
            Self::UseGlobalUnused => Scope::Repo,
            Self::UseLocalDescMissing => Scope::Package,
            Self::UseLocalGlobal => Scope::Package,
            Self::UseLocalUnsorted => Scope::Package,
            Self::UseLocalUnused => Scope::Package,
            Self::VariableOrder => Scope::Version,
            Self::WhitespaceInvalid => Scope::Version,
            Self::WhitespaceUnneeded => Scope::Version,
        }
    }

    /// Determine if a report is disabled for a scanning run due to scan scope.
    pub(crate) fn scoped(&self, scope: Scope) -> Option<Scope> {
        if self.scope() > scope {
            Some(self.scope())
        } else {
            None
        }
    }

    /// Return true if the report supports post-run finalization for a scope.
    pub(crate) fn finish_check(&self, scope: Scope) -> bool {
        match self {
            Self::ArchesUnused => scope == Scope::Repo,
            Self::EapiUnused => scope == Scope::Repo,
            Self::EclassUnused => scope == Scope::Repo,
            Self::LicensesUnused => scope == Scope::Repo,
            Self::IgnoreUnused => scope == Scope::Repo,
            Self::ManifestCollide => scope == Scope::Repo,
            Self::ManifestConflict => scope == Scope::Repo,
            Self::MirrorsUnused => scope == Scope::Repo,
            Self::PackageDeprecatedUnused => scope == Scope::Repo,
            Self::UseGlobalUnused => scope == Scope::Repo,
            _ => false,
        }
    }

    /// Return true if the report supports post-run finalization for a target.
    pub(crate) fn finish_target(&self) -> bool {
        matches!(self, Self::IgnoreUnused)
    }

    /// Return the sorted set of reports enabled by default for an ebuild repo.
    pub fn defaults(repo: &EbuildRepo) -> IndexSet<Self> {
        let mut set: IndexSet<_> = Check::iter_default(repo)
            .flat_map(|x| x.reports())
            .copied()
            .collect();
        set.sort();
        set
    }

    /// Return the sorted set of supported reports for an ebuild repo.
    pub fn supported<T: Into<Scope>>(repo: &EbuildRepo, value: T) -> IndexSet<Self> {
        let scope = value.into();
        let mut set: IndexSet<_> = Check::iter_supported(repo, scope)
            .flat_map(|c| c.reports())
            .filter(|r| scope >= r.scope())
            .copied()
            .collect();
        set.sort();
        set
    }
}

/// Builder for reports.
pub(crate) struct ReportBuilder(Report);

impl ReportBuilder {
    /// Add a report message.
    pub(crate) fn message<S>(mut self, value: S) -> Self
    where
        S: fmt::Display,
    {
        self.0.message = Some(value.to_string());
        self
    }

    /// Add a location reference.
    pub(crate) fn location<L>(mut self, value: L) -> Self
    where
        L: Into<Location>,
    {
        if let ReportScope::Version(_, location @ None) = &mut self.0.scope {
            *location = Some(value.into());
        } else {
            panic!("invalid report scope: {:?}", self.0.scope);
        }

        self
    }

    /// Pass the report to the scanning filter for processing.
    pub(crate) fn report(self, filter: &ReportFilter) {
        filter.report(self.0)
    }
}

/// A position in a multi-line text file, in terms of lines and columns.
///
/// Values are not zero-based so a value of zero means the field is unset.
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line {}", self.line)?;
        if self.column > 0 {
            write!(f, ", column {}", self.column)?;
        }
        Ok(())
    }
}

impl From<usize> for Location {
    fn from(value: usize) -> Self {
        Self { line: value, column: 0 }
    }
}

impl From<(usize, usize)> for Location {
    fn from(value: (usize, usize)) -> Self {
        Self { line: value.0, column: value.1 }
    }
}

impl From<&Node<'_>> for Location {
    fn from(value: &Node<'_>) -> Self {
        Self {
            line: value.start_position().row + 1,
            column: value.start_position().column + 1,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum ReportScope {
    Version(Cpv, Option<Location>),
    Package(Cpn),
    Category(String),
    Repo(String),
}

impl ReportScope {
    fn scope(&self) -> Scope {
        match self {
            Self::Version(_, _) => Scope::Version,
            Self::Package(_) => Scope::Package,
            Self::Category(_) => Scope::Category,
            Self::Repo(_) => Scope::Repo,
        }
    }
}

impl Ord for ReportScope {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Repo(v1), Self::Repo(v2)) => v1.cmp(v2),
            (Self::Category(v1), Self::Category(v2)) => v1.cmp(v2),
            (Self::Package(v1), Self::Package(v2)) => v1.cmp(v2),
            (Self::Version(v1, l1), Self::Version(v2, l2)) => {
                v1.cmp(v2).then_with(|| l1.cmp(l2))
            }
            (Self::Version(v1, _), Self::Package(v2)) => v1
                .cpn()
                .cmp(v2)
                .then_with(|| self.scope().cmp(&other.scope())),
            (Self::Package(v1), Self::Version(v2, _)) => v1
                .cmp(v2.cpn())
                .then_with(|| self.scope().cmp(&other.scope())),
            _ => self.scope().cmp(&other.scope()),
        }
    }
}

impl PartialOrd for ReportScope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for ReportScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Version(cpv, Some(location)) => {
                write!(f, "Version( {cpv}, {location:?} )")
            }
            Self::Version(cpv, None) => write!(f, "Version( {cpv} )"),
            Self::Package(cpn) => write!(f, "Package( {cpn} )"),
            Self::Category(cat) => write!(f, "Category( {cat} )"),
            Self::Repo(repo) => write!(f, "Repo( {repo} )"),
        }
    }
}

impl fmt::Display for ReportScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Version(cpv, Some(location)) => write!(f, "{cpv}, {location}"),
            Self::Version(cpv, None) => write!(f, "{cpv}"),
            Self::Package(cpn) => write!(f, "{cpn}"),
            Self::Category(cat) => write!(f, "{cat}/*"),
            Self::Repo(repo) => write!(f, "{repo}"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct Report {
    pub kind: ReportKind,
    scope: ReportScope,
    message: Option<String>,
}

impl Report {
    /// The scope the report relates to, e.g. a specific package version or package name.
    pub fn scope(&self) -> &ReportScope {
        &self.scope
    }

    /// The report message.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The severity of the report.
    pub fn level(&self) -> ReportLevel {
        self.kind.level()
    }

    /// Serialize a [`Report`] into a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).expect("failed serializing report")
    }

    /// Deserialize a JSON string into a [`Report`].
    pub fn from_json(data: &str) -> crate::Result<Self> {
        serde_json::from_str(data)
            .map_err(|e| Error::InvalidValue(format!("failed deserializing report: {e}")))
    }
}

impl Ord for Report {
    fn cmp(&self, other: &Self) -> Ordering {
        self.scope
            .cmp(&other.scope)
            .then_with(|| self.kind.cmp(&other.kind))
            .then_with(|| self.message.cmp(&other.message))
    }
}

impl PartialOrd for Report {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.scope, self.kind)?;
        if let Some(value) = self.message() {
            write!(f, ": {value}")?;
        }
        Ok(())
    }
}

impl Restriction<&Report> for Restrict {
    fn matches(&self, report: &Report) -> bool {
        match &report.scope {
            ReportScope::Version(cpv, _) => self.matches(cpv),
            ReportScope::Package(cpn) => self.matches(cpn),
            _ => false,
        }
    }
}

/// Iterator for deserializing reports from a BufRead object.
pub struct Iter<'a, R: BufRead> {
    reader: R,
    line: String,
    reports: Option<&'a IndexSet<ReportKind>>,
    restrict: Option<&'a Restrict>,
    scopes: Option<&'a IndexSet<Scope>>,
}

impl<'a> Iter<'a, BufReader<File>> {
    /// Try to create a new reports iterator from a file path.
    pub fn try_from_file<P: AsRef<Utf8Path>>(
        path: P,
        reports: Option<&'a IndexSet<ReportKind>>,
        restrict: Option<&'a Restrict>,
        scopes: Option<&'a IndexSet<Scope>>,
    ) -> crate::Result<Iter<'a, BufReader<File>>> {
        let path = path.as_ref();
        let file = File::open(path)
            .map_err(|e| Error::InvalidValue(format!("failed loading file: {path}: {e}")))?;
        Ok(Iter {
            reader: BufReader::new(file),
            line: String::new(),
            reports,
            restrict,
            scopes,
        })
    }
}

impl<'a, R: BufRead> Iter<'a, R> {
    /// Create a new reports iterator from a BufRead object.
    pub fn from_reader(
        reader: R,
        reports: Option<&'a IndexSet<ReportKind>>,
        restrict: Option<&'a Restrict>,
        scopes: Option<&'a IndexSet<Scope>>,
    ) -> Iter<'a, R> {
        Iter {
            reader,
            line: String::new(),
            reports,
            restrict,
            scopes,
        }
    }

    /// Determine if a given [`Report`] should be filtered.
    fn filtered(&self, report: &Report) -> bool {
        // skip excluded report variants
        if let Some(reports) = self.reports {
            if !reports.contains(&report.kind) {
                return true;
            }
        }

        // skip excluded scope variants
        if let Some(scopes) = self.scopes {
            if !scopes.contains(&report.scope().scope()) {
                return true;
            }
        }

        // skip excluded restrictions
        if let Some(filter) = self.restrict {
            if !filter.matches(report) {
                return true;
            }
        }

        false
    }
}

impl<R: BufRead> Iterator for Iter<'_, R> {
    type Item = crate::Result<Report>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.line.clear();
            match self.reader.read_line(&mut self.line) {
                Ok(0) => return None,
                Ok(_) => match Report::from_json(&self.line) {
                    Ok(report) => {
                        if !self.filtered(&report) {
                            return Some(Ok(report));
                        }
                    }
                    err => return Some(err),
                },
                Err(e) => {
                    return Some(Err(Error::InvalidValue(format!("failed reading line: {e}"))))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use pkgcraft::restrict::Scope;
    use pkgcraft::test::*;
    use pretty_assertions::assert_eq;
    use strum::IntoEnumIterator;

    use super::*;
    use crate::check::Check;
    use crate::report::ReportLevel;

    // serialized reports in order
    static REPORTS: &str = indoc::indoc! {r#"
        {"kind":"DependencyDeprecated","scope":{"Version":["cat/pkg1-2-r3",null]},"message":"BDEPEND: cat/deprecated"}
        {"kind":"EapiDeprecated","scope":{"Version":["cat/pkg1-2-r3",null]},"message":"6"}
        {"kind":"WhitespaceUnneeded","scope":{"Version":["cat/pkg1-2-r3",{"line":3,"column":0}]},"message":"empty line"}
        {"kind":"WhitespaceInvalid","scope":{"Version":["cat/pkg1-2-r3",{"line":6,"column":0}]},"message":"missing ending newline"}
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg1"},"message":"arch1"}
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg1"},"message":"arch2"}
        {"kind":"EapiDeprecated","scope":{"Version":["cat/pkg2-1-r2",null]},"message":"6"}
        {"kind":"RepoCategoryEmpty","scope":{"Category":"cat1"},"message":null}
        {"kind":"RepoCategoryEmpty","scope":{"Category":"cat2"},"message":null}
        {"kind":"LicensesUnused","scope":{"Repo":"repo1"},"message":"unused"}
        {"kind":"LicensesUnused","scope":{"Repo":"repo2"},"message":"unused"}
    "#};

    #[test]
    fn kind() {
        // verify ReportKind are kept in lexical order
        let kinds: Vec<_> = ReportKind::iter().collect();
        let ordered: Vec<_> = ReportKind::iter().map(|x| x.to_string()).sorted().collect();
        let ordered: Vec<_> = ordered.iter().map(|s| s.parse().unwrap()).collect();
        assert_eq!(&kinds, &ordered, "unordered ReportKind variants");
    }

    #[test]
    fn cmp() {
        // deserialize reports
        let expected: Vec<_> = REPORTS
            .lines()
            .map(Report::from_json)
            .try_collect()
            .unwrap();

        // verify ordering manually for PartialOrd tests
        for (a, b) in expected.iter().tuples() {
            assert!(a < b);
            assert!(a.scope() <= b.scope());
        }

        // reverse reports and sort them back into the expected order
        let mut reports = expected.clone();
        reports.reverse();
        reports.sort();

        // compare reports via string serialization for better diff output
        let expected = expected.iter().map(|r| r.to_string());
        let reports = reports.iter().map(|r| r.to_string());
        assert_ordered_eq!(expected, reports);
    }

    #[test]
    fn display_and_debug() {
        for report in REPORTS.lines().filter_map(|s| Report::from_json(s).ok()) {
            let kind = report.kind.to_string();
            let scope = report.scope().to_string();

            // regular output
            let s = report.to_string();
            assert!(s.contains(&kind));
            assert!(s.contains(&scope));

            // debug output
            let s = format!("{report:?}");
            assert!(s.contains(&kind));
        }
    }

    #[test]
    fn builder() {
        // location is only valid for version scope reports
        let result = std::panic::catch_unwind(|| {
            let cpv = Cpv::try_new("cat/pkg-1").unwrap();
            ReportKind::Builtin.version(cpv).location(1)
        });
        assert!(result.is_ok());
        let result = std::panic::catch_unwind(|| {
            let cpn = Cpn::try_new("cat/pkg").unwrap();
            ReportKind::LiveOnly.package(cpn).location(1)
        });
        assert!(result.is_err());
    }

    #[test]
    fn report_target() {
        let data = test_data();

        // default checks for gentoo repo
        let repo = data.ebuild_repo("gentoo").unwrap();
        let defaults = ReportKind::defaults(repo);
        let supported = ReportKind::supported(repo, Scope::Repo);
        let (enabled, _) = ReportTarget::collapse([], &defaults, &supported).unwrap();
        let checks: IndexSet<_> = Check::iter_report(&enabled).collect();
        // repo specific checks enabled when scanning the matching repo
        assert!(checks.contains(&Check::Header));

        // default checks
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let defaults = ReportKind::defaults(repo);
        let supported = ReportKind::supported(repo, Scope::Repo);
        let (enabled, _) = ReportTarget::collapse([], &defaults, &supported).unwrap();
        let checks: IndexSet<_> = Check::iter_report(&enabled).collect();
        assert!(checks.contains(&Check::Dependency));
        // optional checks aren't run by default when scanning
        assert!(!checks.contains(&Check::UnstableOnly));
        // repo specific checks aren't run by default when scanning non-matching repo
        assert!(!checks.contains(&Check::Header));

        // non-default reports aren't enabled when their matching level is targeted
        let report = ReportKind::HeaderInvalid;
        assert_eq!(report.level(), ReportLevel::Error);
        let target = ReportLevel::Error.into();
        let (enabled, _) = ReportTarget::collapse([&target], &defaults, &supported).unwrap();
        assert!(!enabled.contains(&report));
        assert!(!enabled.is_empty());
    }
}
