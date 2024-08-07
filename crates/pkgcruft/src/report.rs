use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};

use camino::Utf8Path;
use colored::Color;
use pkgcraft::dep::{Cpn, Cpv};
use pkgcraft::pkg::Package;
use pkgcraft::repo::Repository;
use pkgcraft::restrict::{Restrict, Restriction};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

use crate::scanner::ReportFilter;
use crate::scope::Scope;
use crate::Error;

/// The severity of the report.
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

/// Report variants.
#[derive(
    Serialize,
    Deserialize,
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
pub enum ReportKind {
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

    /// Eclass that is unused in the parent repository.
    EclassUnused,

    /// File has an invalid copyright and/or license header.
    HeaderInvalid,

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

    /// Ebuild has invalid LICENSE.
    LicenseInvalid,

    /// Ebuild has a missing license.
    LicenseMissing,

    /// Ebuild has an unneeded license.
    LicenseUnneeded,

    /// Package only has live ebuilds.
    LiveOnly,

    /// Ebuild is missing mandatory metadata values.
    MetadataMissing,

    /// Overlay package matches the name of a package from a parent repo.
    PackageOverride,

    /// Ebuild has invalid PROPERTIES.
    PropertiesInvalid,

    /// Ebuild can support newer python version(s).
    PythonUpdate,

    /// Ebuild has invalid REQUIRED_USE.
    RequiredUseInvalid,

    /// Ebuild has invalid RESTRICT.
    RestrictInvalid,

    /// Ebuild is missing a RESTRICT value of the specified type.
    RestrictMissing,

    /// Ebuild can support newer ruby version(s).
    RubyUpdate,

    /// Ebuild has bash failures when sourced.
    SourcingError,

    /// Package only has unstable keywords.
    UnstableOnly,

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
    pub(crate) fn version<P: Package>(self, pkg: P) -> ReportBuilder {
        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Version(pkg.cpv().clone(), None),
            message: Default::default(),
        })
    }

    /// Create a package scope report.
    pub(crate) fn package(self, cpn: &Cpn) -> ReportBuilder {
        ReportBuilder(Report {
            kind: self,
            scope: ReportScope::Package(cpn.clone()),
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
            Self::DependencyDeprecated => Warning,
            Self::DependencyInvalid => Critical,
            Self::DependencyRevisionMissing => Warning,
            Self::DependencySlotMissing => Warning,
            Self::EapiBanned => Error,
            Self::EapiDeprecated => Warning,
            Self::EapiFormat => Style,
            Self::EapiStale => Warning,
            Self::EapiUnstable => Error,
            Self::EclassUnused => Warning,
            Self::HeaderInvalid => Error,
            Self::KeywordsDropped => Warning,
            Self::KeywordsLive => Warning,
            Self::KeywordsOverlapping => Error,
            Self::KeywordsUnsorted => Style,
            Self::LicenseDeprecated => Warning,
            Self::LicenseInvalid => Critical,
            Self::LicenseMissing => Error,
            Self::LicenseUnneeded => Warning,
            Self::LiveOnly => Warning,
            Self::MetadataMissing => Critical,
            Self::PackageOverride => Warning,
            Self::PropertiesInvalid => Critical,
            Self::PythonUpdate => Info,
            Self::RequiredUseInvalid => Critical,
            Self::RestrictInvalid => Critical,
            Self::RestrictMissing => Warning,
            Self::RubyUpdate => Info,
            Self::SourcingError => Critical,
            Self::UnstableOnly => Info,
            Self::UseLocalDescMissing => Error,
            Self::UseLocalGlobal => Warning,
            Self::UseLocalUnused => Warning,
            Self::UseLocalUnsorted => Style,
            Self::VariableOrder => Style,
            Self::WhitespaceInvalid => Warning,
            Self::WhitespaceUnneeded => Style,
        }
    }
}

/// Builder for reports.
pub(crate) struct ReportBuilder(Report);

impl ReportBuilder {
    /// Add a report message.
    pub(crate) fn message<S>(&mut self, value: S) -> &mut Self
    where
        S: fmt::Display,
    {
        self.0.message = value.to_string();
        self
    }

    /// Add a location reference.
    pub(crate) fn location<L>(&mut self, value: L) -> &mut Self
    where
        L: Into<Location>,
    {
        match &mut self.0.scope {
            ReportScope::Version(_, location @ None) => *location = Some(value.into()),
            _ => unreachable!("invalid report scope: {:?}", self.0.scope),
        }
        self
    }

    /// Pass the report to the scanning filter for processing.
    pub(crate) fn report(&mut self, filter: &mut ReportFilter) {
        filter.report(&self.0)
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
            (Self::Version(v1, l1), Self::Version(v2, l2)) => v1.cmp(v2).then_with(|| l1.cmp(l2)),
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
            Self::Version(cpv, Some(location)) => write!(f, "Version( {cpv}, {location:?} )"),
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
            Self::Category(cat) => write!(f, "{cat}"),
            Self::Repo(repo) => write!(f, "{repo}"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct Report {
    kind: ReportKind,
    scope: ReportScope,
    message: String,
}

impl Report {
    /// The report variant.
    pub fn kind(&self) -> &ReportKind {
        &self.kind
    }

    /// The scope the report relates to, e.g. a specific package version or package name.
    pub fn scope(&self) -> &ReportScope {
        &self.scope
    }

    /// The report message.
    pub fn message(&self) -> &str {
        &self.message
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
        if !self.message.is_empty() {
            write!(f, ": {}", self.message)?;
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
    reports: Option<&'a HashSet<ReportKind>>,
    restrict: Option<&'a Restrict>,
}

impl<'a> Iter<'a, BufReader<File>> {
    /// Try to create a new reports iterator from a file path.
    pub fn try_from_file<P: AsRef<Utf8Path>>(
        path: P,
        reports: Option<&'a HashSet<ReportKind>>,
        restrict: Option<&'a Restrict>,
    ) -> crate::Result<Iter<'a, BufReader<File>>> {
        let path = path.as_ref();
        let file = File::open(path)
            .map_err(|e| Error::InvalidValue(format!("failed loading file: {path}: {e}")))?;
        Ok(Iter {
            reader: BufReader::new(file),
            line: String::new(),
            reports,
            restrict,
        })
    }
}

impl<'a, R: BufRead> Iter<'a, R> {
    /// Create a new reports iterator from a BufRead object.
    pub fn from_reader(
        reader: R,
        reports: Option<&'a HashSet<ReportKind>>,
        restrict: Option<&'a Restrict>,
    ) -> Iter<'a, R> {
        Iter {
            reader,
            line: String::new(),
            reports,
            restrict,
        }
    }

    /// Determine if a given [`Report`] should be filtered.
    fn filtered(&self, report: &Report) -> bool {
        // skip excluded report variants
        if let Some(reports) = self.reports {
            if !reports.contains(report.kind()) {
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
    use pretty_assertions::assert_eq;
    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn kind() {
        // verify ReportKind are kept in lexical order
        let kinds: Vec<_> = ReportKind::iter().collect();
        let ordered: Vec<_> = ReportKind::iter().map(|x| x.to_string()).sorted().collect();
        let ordered: Vec<_> = ordered.iter().map(|s| s.parse().unwrap()).collect();
        assert_eq!(&kinds, &ordered);
    }

    #[rustfmt::skip]
    #[test]
    fn cmp() {
        // serialized reports in order
        let data = indoc::indoc! {r#"
            {"kind":"DependencyDeprecated","scope":{"Version":["cat/pkg1-2-r3",null]},"message":"BDEPEND: cat/deprecated"}
            {"kind":"EapiDeprecated","scope":{"Version":["cat/pkg1-2-r3",null]},"message":"6"}
            {"kind":"WhitespaceUnneeded","scope":{"Version":["cat/pkg1-2-r3",{"line":3,"column":0}]},"message":"empty line"}
            {"kind":"WhitespaceInvalid","scope":{"Version":["cat/pkg1-2-r3",{"line":6,"column":0}]},"message":"missing ending newline"}
            {"kind":"UnstableOnly","scope":{"Package":"cat/pkg1"},"message":"arch1"}
            {"kind":"UnstableOnly","scope":{"Package":"cat/pkg1"},"message":"arch2"}
            {"kind":"EapiDeprecated","scope":{"Version":["cat/pkg2-1-r2",null]},"message":"6"}
        "#};

        // reverse reports and sort them back into the expected order
        let expected: Vec<_> = data.lines().filter_map(|s| Report::from_json(s).ok()).collect();
        let mut reports = expected.clone();
        reports.reverse();
        reports.sort();

        // compare reports via string serialization for better diff output
        let expected: Vec<_> = expected.iter().map(|r| r.to_string()).collect();
        let reports: Vec<_> = reports.iter().map(|r| r.to_string()).collect();
        assert_eq!(&expected, &reports);
    }
}
