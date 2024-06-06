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
    Hash,
    Copy,
    Clone,
)]
pub enum ReportKind {
    DependencyDeprecated,
    DependencyInvalid,
    DependencySlotMissing,

    /// Package has a banned EAPI.
    EapiBanned,

    /// Package has a deprecated EAPI.
    EapiDeprecated,

    /// Package has an older EAPI than the previous release in the same SLOT.
    EapiStale,

    /// Package has stable keywords with an unstable EAPI.
    EapiUnstable,

    /// Eclass that is unused in the parent repository.
    EclassUnused,

    /// File has an invalid copyright and/or license header.
    HeaderInvalid,

    KeywordsDropped,
    KeywordsOverlapping,
    KeywordsUnsorted,
    LicenseInvalid,

    /// Package only has live ebuilds.
    LiveOnly,

    MetadataMissing,
    PropertiesInvalid,
    RequiredUseInvalid,
    RestrictInvalid,
    RestrictMissing,
    RevisionMissing,
    SourcingError,
    UnstableOnly,

    /// Local USE flag missing description.
    UseLocalDescMissing,

    /// Local USE flag description matching a global USE flag.
    UseLocalGlobal,

    /// Local USE flag that is unsorted.
    UseLocalUnsorted,

    /// Local USE flag that is unused.
    UseLocalUnused,
}

impl Ord for ReportKind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for ReportKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl ReportKind {
    /// Create a version scope report.
    pub(crate) fn version<P, S>(self, pkg: P, message: S) -> Report
    where
        P: Package,
        S: fmt::Display,
    {
        Report {
            kind: self,
            scope: ReportScope::Version(pkg.cpv().clone()),
            message: message.to_string(),
        }
    }

    /// Create a package scope report.
    pub(crate) fn package<P, S>(self, pkgs: &[P], message: S) -> Report
    where
        P: Package,
        S: fmt::Display,
    {
        Report {
            kind: self,
            scope: ReportScope::Package(pkgs[0].cpn().clone()),
            message: message.to_string(),
        }
    }

    /// Create a repo scope report.
    pub(crate) fn repo<R, S>(self, repo: R, message: S) -> Report
    where
        R: Repository,
        S: fmt::Display,
    {
        Report {
            kind: self,
            scope: ReportScope::Repo(repo.name().to_string()),
            message: message.to_string(),
        }
    }

    /// Return the severity level of the report variant.
    pub fn level(&self) -> ReportLevel {
        use ReportLevel::*;
        match self {
            Self::DependencyDeprecated => Warning,
            Self::DependencyInvalid => Critical,
            Self::DependencySlotMissing => Warning,
            Self::EapiBanned => Error,
            Self::EapiDeprecated => Warning,
            Self::EapiStale => Warning,
            Self::EapiUnstable => Error,
            Self::EclassUnused => Warning,
            Self::HeaderInvalid => Error,
            Self::KeywordsDropped => Warning,
            Self::KeywordsOverlapping => Error,
            Self::KeywordsUnsorted => Style,
            Self::LicenseInvalid => Critical,
            Self::LiveOnly => Warning,
            Self::MetadataMissing => Critical,
            Self::PropertiesInvalid => Critical,
            Self::RequiredUseInvalid => Critical,
            Self::RestrictInvalid => Critical,
            Self::RestrictMissing => Warning,
            Self::RevisionMissing => Warning,
            Self::SourcingError => Critical,
            Self::UnstableOnly => Info,
            Self::UseLocalDescMissing => Error,
            Self::UseLocalGlobal => Warning,
            Self::UseLocalUnused => Warning,
            Self::UseLocalUnsorted => Style,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum ReportScope {
    Version(Cpv),
    Package(Cpn),
    Category(String),
    Repo(String),
}

impl ReportScope {
    fn scope(&self) -> Scope {
        match self {
            Self::Version(_) => Scope::Version,
            Self::Package(_) => Scope::Package,
            Self::Category(_) => Scope::Category,
            Self::Repo(_) => Scope::Repo,
        }
    }
}

impl Ord for ReportScope {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Repo(repo1), Self::Repo(repo2)) => repo1.cmp(repo2),
            (Self::Category(cat1), Self::Category(cat2)) => cat1.cmp(cat2),
            (Self::Package(cpn1), Self::Package(cpn2)) => cpn1.cmp(cpn2),
            (Self::Version(cpv1), Self::Version(cpv2)) => cpv1.cmp(cpv2),
            (Self::Version(cpv), Self::Package(cpn)) => cpv.cpn().cmp(cpn).then(Ordering::Less),
            (Self::Package(cpn), Self::Version(cpv)) => cpn.cmp(cpv.cpn()).then(Ordering::Greater),
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
            Self::Version(cpv) => write!(f, "Version( {cpv} )"),
            Self::Package(cpn) => write!(f, "Package( {cpn} )"),
            Self::Category(cat) => write!(f, "Category( {cat} )"),
            Self::Repo(repo) => write!(f, "Repo( {repo} )"),
        }
    }
}

impl fmt::Display for ReportScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Version(cpv) => write!(f, "{cpv}"),
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
        write!(f, "{}: {}: {}", self.scope, self.kind, self.message)
    }
}

impl Restriction<&Report> for Restrict {
    fn matches(&self, report: &Report) -> bool {
        match &report.scope {
            ReportScope::Version(cpv) => self.matches(cpv),
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
    use super::*;

    #[rustfmt::skip]
    #[test]
    fn cmp() {
        let pkg_r1 = Report::from_json(r#"{"kind":"UnstableOnly","scope":{"Package":"cat/pkg1"},"message":"arch1"}"#).unwrap();
        let pkg_r2 = Report::from_json(r#"{"kind":"UnstableOnly","scope":{"Package":"cat/pkg1"},"message":"arch2"}"#).unwrap();
        let ver_r3 = Report::from_json(r#"{"kind":"DependencyDeprecated","scope":{"Version":"cat/pkg1-2-r3"},"message":"BDEPEND: cat/deprecated"}"#).unwrap();
        let ver_r4 = Report::from_json(r#"{"kind":"EapiDeprecated","scope":{"Version":"cat/pkg1-2-r3"},"message":"6"}"#).unwrap();
        let ver_r5 = Report::from_json(r#"{"kind":"EapiDeprecated","scope":{"Version":"cat/pkg2-1-r2"},"message":"6"}"#).unwrap();

        assert!(pkg_r1 == pkg_r1);
        // message ordering
        assert!(pkg_r1 < pkg_r2);
        // scope ordering
        assert!(ver_r3 < pkg_r2);
        assert!(pkg_r2 > ver_r4);
        // package ordering
        assert!(ver_r5 > pkg_r2);
        assert!(ver_r5 > ver_r4);
        // report ordering
        assert!(ver_r3 < ver_r4);
    }
}
