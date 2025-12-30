use std::cmp::Ordering;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use owo_colors::OwoColorize;
use pkgcraft::bash::Node;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::repo::{EbuildRepo, Repository};
use pkgcraft::restrict::{Restrict, Restriction, Scope};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString};

use crate::Error;
use crate::check::Check;
use crate::scan::ScannerRun;

mod set;
pub use set::ReportSet;
mod target;
pub use target::ReportTarget;

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

    /// An EAPI command uses `|| die` which is unneeded.
    CommandDieUnneeded,

    /// An EAPI command is used in an invalid scope.
    CommandScopeInvalid,

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

    /// Repo has an invalid ignore directive.
    IgnoreInvalid,

    /// Repo has an unused ignore directive.
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

    /// An ebuild phase is directly called.
    PhaseCall,

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

    /// An EAPI variable is used in an invalid scope.
    VariableScopeInvalid,

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
        T::Error: fmt::Display,
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

    /// Create a report using a scope.
    pub(crate) fn in_scope(self, scope: ReportScope) -> ReportBuilder {
        ReportBuilder(Report {
            kind: self,
            scope,
            message: Default::default(),
        })
    }

    /// Return the severity level of the report variant.
    pub fn level(&self) -> ReportLevel {
        use ReportLevel::*;
        match self {
            Self::ArchesUnused => Warning,
            Self::Builtin => Error,
            Self::CommandDieUnneeded => Warning,
            Self::CommandScopeInvalid => Error,
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
            Self::IgnoreInvalid => Warning,
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
            Self::PhaseCall => Error,
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
            Self::VariableScopeInvalid => Error,
            Self::WhitespaceInvalid => Warning,
            Self::WhitespaceUnneeded => Style,
        }
    }

    /// Render the report variant into a string using its defined level color.
    pub fn colorize(&self) -> String {
        let s = self.as_ref();
        match self.level() {
            ReportLevel::Critical => s.red().to_string(),
            ReportLevel::Error => s.fg_rgb::<255, 140, 0>().to_string(),
            ReportLevel::Warning => s.yellow().to_string(),
            ReportLevel::Style => s.cyan().to_string(),
            ReportLevel::Info => s.green().to_string(),
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
            Self::CommandDieUnneeded => Scope::Version,
            Self::CommandScopeInvalid => Scope::Version,
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
            Self::IgnoreInvalid => Scope::Version,
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
            Self::PhaseCall => Scope::Version,
            Self::PropertiesInvalid => Scope::Version,
            Self::PythonUpdate => Scope::Version,
            Self::RepoCategoriesUnused => Scope::Repo,
            Self::RepoCategoryEmpty => Scope::Repo,
            Self::RepoPackageEmpty => Scope::Package,
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
            Self::VariableScopeInvalid => Scope::Version,
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
            Self::RepoCategoryEmpty => scope == Scope::Repo,
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
            .flat_map(|x| x.reports)
            .copied()
            .collect();
        set.sort_unstable();
        set
    }

    /// Return the sorted set of supported reports for an ebuild repo.
    pub fn supported<T: Into<Scope>>(repo: &EbuildRepo, value: T) -> IndexSet<Self> {
        let scope = value.into();
        let mut set: IndexSet<_> = Check::iter_supported(repo, scope)
            .flat_map(|c| c.reports)
            .filter(|r| scope >= r.scope())
            .copied()
            .collect();
        set.sort_unstable();
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

    /// Queue the report for processing.
    pub(crate) fn report(self, run: &ScannerRun) {
        run.report(self.0)
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

    /// Convert scope to its absolute repo path.
    pub(crate) fn to_abspath<R: Repository>(&self, repo: R) -> Utf8PathBuf {
        repo.path().join(self.to_relpath())
    }

    /// Convert scope to its relative repo path.
    pub(crate) fn to_relpath(&self) -> Utf8PathBuf {
        match self {
            Self::Version(cpv, _) => cpv.relpath(),
            Self::Package(cpn) => cpn.to_string().into(),
            Self::Category(category) => category.into(),
            Self::Repo(_) => Default::default(),
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

impl PartialEq<Scope> for ReportScope {
    fn eq(&self, other: &Scope) -> bool {
        self.scope() == *other
    }
}

impl PartialOrd<Scope> for ReportScope {
    fn partial_cmp(&self, other: &Scope) -> Option<Ordering> {
        Some(self.scope().cmp(other))
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Report {
    scope: ReportScope,
    pub kind: ReportKind,
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

    /// Serialize a [`Report`] into a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).expect("failed serializing report")
    }

    /// Deserialize a JSON string into a [`Report`].
    pub fn from_json(data: &str) -> crate::Result<Self> {
        serde_json::from_str(data).map_err(|e| {
            Error::InvalidValue(format!("failed deserializing report JSON: {data}: {e}"))
        })
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
        if let Some(reports) = self.reports
            && !reports.contains(&report.kind)
        {
            return true;
        }

        // skip excluded scope variants
        if let Some(scopes) = self.scopes
            && !scopes.contains(&report.scope().scope())
        {
            return true;
        }

        // skip excluded restrictions
        if let Some(filter) = self.restrict
            && !filter.matches(report)
        {
            return true;
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
                    return Some(Err(Error::InvalidValue(format!(
                        "failed reading line: {e}"
                    ))));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use strum::IntoEnumIterator;

    use crate::test::assert_ordered_reports;

    use super::*;

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

        assert_ordered_reports!(expected, reports);
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
}
