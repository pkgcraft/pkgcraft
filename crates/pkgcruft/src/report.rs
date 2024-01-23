use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::str::FromStr;

use colored::Color;
use indexmap::IndexSet;
use once_cell::sync::Lazy;
use pkgcraft::dep::{Cpv, Dep};
use pkgcraft::macros::cmp_not_equal;
use pkgcraft::pkg::Package;
use pkgcraft::restrict::{Restrict, Restriction};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString};

use crate::check::CHECKS;
use crate::Error;

/// The severity of the report.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum ReportLevel {
    Error,
    Warning,
    Style,
    Info,
}

impl From<ReportLevel> for Color {
    fn from(level: ReportLevel) -> Self {
        match level {
            ReportLevel::Error => Color::Red,
            ReportLevel::Warning => Color::Yellow,
            ReportLevel::Style => Color::Cyan,
            ReportLevel::Info => Color::Green,
        }
    }
}

/// Report variants that relate to ebuild packages.
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
pub enum VersionReport {
    DeprecatedDependency,
    DroppedKeywords,
    InvalidDependencySet,
    MissingMetadata,
    MissingRevision,
    SourcingError,
}

impl VersionReport {
    pub(crate) fn report<P, S>(self, pkg: P, description: S) -> Report
    where
        P: Package,
        S: Into<String>,
    {
        Report {
            scope: ReportScope::Version(pkg.cpv().clone()),
            kind: ReportKind::Version(self),
            description: description.into(),
        }
    }

    fn level(&self) -> ReportLevel {
        use ReportLevel::*;
        match self {
            Self::DeprecatedDependency => Warning,
            Self::DroppedKeywords => Warning,
            Self::InvalidDependencySet => Error,
            Self::MissingMetadata => Error,
            Self::MissingRevision => Warning,
            Self::SourcingError => Error,
        }
    }
}

/// Report variants that relate to ebuild package sets.
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
pub enum PackageReport {
    UnstableOnly,
}

impl PackageReport {
    pub(crate) fn report<P, S>(self, pkgs: &[P], description: S) -> Report
    where
        P: Package,
        S: Into<String>,
    {
        Report {
            scope: ReportScope::Package(pkgs[0].cpn()),
            kind: ReportKind::Package(self),
            description: description.into(),
        }
    }

    fn level(&self) -> ReportLevel {
        use ReportLevel::*;
        match self {
            Self::UnstableOnly => Info,
        }
    }
}

/// All report variants separated by scope.
#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum ReportKind {
    Version(VersionReport),
    Package(PackageReport),
}

impl ReportKind {
    /// The severity of the report variant.
    pub fn level(&self) -> ReportLevel {
        match self {
            Self::Version(k) => k.level(),
            Self::Package(k) => k.level(),
        }
    }
}

impl FromStr for ReportKind {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        REPORTS
            .get(s)
            .ok_or_else(|| Error::InvalidValue(format!("invalid report variant: {s}")))
            .copied()
    }
}

impl std::fmt::Display for ReportKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Version(k) => write!(f, "{k}"),
            Self::Package(k) => write!(f, "{k}"),
        }
    }
}

impl AsRef<str> for ReportKind {
    fn as_ref(&self) -> &str {
        match self {
            Self::Version(r) => r.as_ref(),
            Self::Package(r) => r.as_ref(),
        }
    }
}

impl PartialEq for ReportKind {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl Eq for ReportKind {}

impl Hash for ReportKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl Borrow<str> for ReportKind {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub enum ReportScope {
    Version(Cpv<String>),
    Package(Dep<String>),
}

impl std::fmt::Display for ReportScope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Version(cpv) => write!(f, "{cpv}"),
            Self::Package(cpn) => write!(f, "{cpn}"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub struct Report {
    scope: ReportScope,
    kind: ReportKind,
    description: String,
}

impl Report {
    /// The scope the report relates to, e.g. a specific package version or package name.
    pub fn scope(&self) -> &ReportScope {
        &self.scope
    }

    /// The report variant.
    pub fn kind(&self) -> &ReportKind {
        &self.kind
    }

    /// The description of the report.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The severity of the report.
    pub fn level(&self) -> ReportLevel {
        self.kind.level()
    }

    /// Serialize a [`Report`] into a JSON string.
    pub fn to_json(&self) -> crate::Result<String> {
        serde_json::to_string(&self)
            .map_err(|e| Error::InvalidValue(format!("failed serializing report: {e}")))
    }

    /// Deserialize a JSON string into a [`Report`].
    pub fn from_json(data: &str) -> crate::Result<Self> {
        serde_json::from_str(data)
            .map_err(|e| Error::InvalidValue(format!("failed deserializing report: {e}")))
    }
}

impl Ord for Report {
    fn cmp(&self, other: &Self) -> Ordering {
        use ReportScope::*;
        match (&self.scope, &other.scope) {
            (Version(cpv), Package(dep)) => {
                cmp_not_equal!(&(cpv.category(), cpv.package()), &(dep.category(), dep.package()));
                return Ordering::Less;
            }
            (Package(dep), Version(cpv)) => {
                cmp_not_equal!(&(dep.category(), dep.package()), &(cpv.category(), cpv.package()));
                return Ordering::Greater;
            }
            (s1, s2) => cmp_not_equal!(s1, s2),
        }
        cmp_not_equal!(&self.kind, &other.kind);
        self.description.cmp(&other.description)
    }
}

impl PartialOrd for Report {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Restriction<&Report> for Restrict {
    fn matches(&self, report: &Report) -> bool {
        match &report.scope {
            ReportScope::Version(cpv) => self.matches(cpv),
            ReportScope::Package(cpn) => self.matches(cpn),
        }
    }
}

/// Iterator for deserializing reports from a BufRead object.
pub struct Iter<'a, R: BufRead> {
    reader: R,
    line: String,
    filters: Option<(&'a HashSet<ReportKind>, &'a Restrict)>,
}

impl<'a> Iter<'a, BufReader<File>> {
    /// Try to create a new reports iterator from a file path.
    pub fn try_from_file<P: AsRef<Path>>(
        path: P,
        filters: Option<(&'a HashSet<ReportKind>, &'a Restrict)>,
    ) -> crate::Result<Iter<'a, BufReader<File>>> {
        let path = path.as_ref();
        let file = File::open(path)
            .map_err(|e| Error::InvalidValue(format!("failed loading file: {path:?}: {e}")))?;
        Ok(Iter {
            reader: BufReader::new(file),
            line: String::new(),
            filters,
        })
    }
}

impl<'a, R: BufRead> Iter<'a, R> {
    /// Create a new reports iterator from a BufRead object.
    pub fn from_reader(
        reader: R,
        filters: Option<(&'a HashSet<ReportKind>, &'a Restrict)>,
    ) -> Iter<'a, R> {
        Iter {
            reader,
            line: String::new(),
            filters,
        }
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
                        if let Some((reports, filter)) = self.filters {
                            if !reports.contains(report.kind()) || !filter.matches(&report) {
                                continue;
                            }
                        }
                        return Some(Ok(report));
                    }
                    Err(e) => return Some(Err(e)),
                },
                Err(e) => {
                    return Some(Err(Error::InvalidValue(format!("failed reading line: {e}"))))
                }
            }
        }
    }
}

/// The ordered set of all report variants.
pub static REPORTS: Lazy<IndexSet<ReportKind>> = Lazy::new(|| {
    let mut reports: IndexSet<_> = CHECKS.iter().flat_map(|c| c.reports()).copied().collect();
    reports.sort();
    reports
});
