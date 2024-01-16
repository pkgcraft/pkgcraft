use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use colored::Color;
use indexmap::IndexSet;
use once_cell::sync::Lazy;
use pkgcraft::dep::{Cpv, Dep};
use pkgcraft::pkg::Package;
use pkgcraft::restrict::{Restrict, Restriction};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString};

use crate::check::CHECKS;
use crate::Error;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum ReportLevel {
    Error,
    Warning,
    Style,
    Info,
}

impl From<&ReportLevel> for Color {
    fn from(level: &ReportLevel) -> Self {
        match level {
            ReportLevel::Error => Color::Red,
            ReportLevel::Warning => Color::Yellow,
            ReportLevel::Style => Color::Cyan,
            ReportLevel::Info => Color::Green,
        }
    }
}

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
    InvalidDependency,
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
            level: ReportLevel::Warning,
            description: description.into(),
        }
    }
}

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
            level: ReportLevel::Warning,
            description: description.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum ReportKind {
    Version(VersionReport),
    Package(PackageReport),
}

impl FromStr for ReportKind {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        REPORTS
            .get(s)
            .ok_or_else(|| Error::InvalidValue(format!("unknown report: {s}")))
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Report {
    scope: ReportScope,
    kind: ReportKind,
    level: ReportLevel,
    description: String,
}

impl Report {
    pub fn scope(&self) -> &ReportScope {
        &self.scope
    }

    pub fn kind(&self) -> &ReportKind {
        &self.kind
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub(crate) fn level(&self) -> &ReportLevel {
        &self.level
    }

    /// Serialize a [`Report`] into JSON.
    pub fn to_json(&self) -> crate::Result<String> {
        serde_json::to_string(&self)
            .map_err(|e| Error::InvalidValue(format!("failed serializing report to JSON: {e}")))
    }

    /// Deserialize a JSON string into a [`Report`].
    pub fn from_json(data: &str) -> crate::Result<Self> {
        serde_json::from_str(data)
            .map_err(|e| Error::InvalidValue(format!("failed deserializing JSON to report: {e}")))
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

pub static REPORTS: Lazy<IndexSet<ReportKind>> =
    Lazy::new(|| CHECKS.iter().flat_map(|c| c.reports()).copied().collect());
