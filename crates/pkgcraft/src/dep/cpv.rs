use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::Error;

use super::version::{ParsedVersion, Revision, Version};
use super::{parse, Dep, Intersects};

pub enum CpvOrDep {
    Cpv(Cpv),
    Dep(Dep),
}

impl FromStr for CpvOrDep {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        if let Ok(val) = Dep::from_str(s) {
            Ok(CpvOrDep::Dep(val))
        } else if let Ok(val) = Cpv::from_str(s) {
            Ok(CpvOrDep::Cpv(val))
        } else {
            Err(Error::InvalidValue(format!("invalid cpv or dep: {s}")))
        }
    }
}

impl fmt::Display for CpvOrDep {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Cpv(val) => write!(f, "{val}"),
            Self::Dep(val) => write!(f, "{val}"),
        }
    }
}

impl Intersects<CpvOrDep> for CpvOrDep {
    fn intersects(&self, other: &CpvOrDep) -> bool {
        use CpvOrDep::*;
        match (self, other) {
            (Cpv(obj1), Cpv(obj2)) => obj1.intersects(obj2),
            (Cpv(obj1), Dep(obj2)) => obj1.intersects(obj2),
            (Dep(obj1), Cpv(obj2)) => obj1.intersects(obj2),
            (Dep(obj1), Dep(obj2)) => obj1.intersects(obj2),
        }
    }
}

impl Intersects<Cpv> for CpvOrDep {
    fn intersects(&self, other: &Cpv) -> bool {
        use CpvOrDep::*;
        match (self, other) {
            (Cpv(obj1), obj2) => obj1.intersects(obj2),
            (Dep(obj1), obj2) => obj1.intersects(obj2),
        }
    }
}

impl Intersects<Dep> for CpvOrDep {
    fn intersects(&self, other: &Dep) -> bool {
        use CpvOrDep::*;
        match (self, other) {
            (Cpv(obj1), obj2) => obj1.intersects(obj2),
            (Dep(obj1), obj2) => obj1.intersects(obj2),
        }
    }
}

/// Parsed package identifier from borrowed input string.
#[derive(Debug)]
pub(crate) struct ParsedCpv<'a> {
    pub(crate) category: &'a str,
    pub(crate) package: &'a str,
    pub(crate) version: ParsedVersion<'a>,
    pub(crate) version_str: &'a str,
}

impl ParsedCpv<'_> {
    pub(crate) fn into_owned(self) -> crate::Result<Cpv> {
        Ok(Cpv {
            category: self.category.to_string(),
            package: self.package.to_string(),
            version: self.version.into_owned(self.version_str)?,
        })
    }
}

/// Package identifier.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cpv {
    category: String,
    package: String,
    version: Version,
}

impl Cpv {
    /// Create a new Cpv from a given string (e.g. cat/pkg-1).
    pub fn new<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        parse::cpv(s.as_ref())
    }

    /// Verify a string represents a valid CPV.
    pub fn valid<S: AsRef<str>>(s: S) -> crate::Result<()> {
        parse::cpv_str(s.as_ref())?;
        Ok(())
    }

    /// Return a Cpv's category.
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Return a Cpv's package.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Return a Cpv's version.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Return a Cpv's revision.
    pub fn revision(&self) -> Option<&Revision> {
        self.version.revision()
    }

    /// Return the package name and version.
    pub fn p(&self) -> String {
        format!("{}-{}", self.package(), self.version.base())
    }

    /// Return the package name, version, and revision.
    pub fn pf(&self) -> String {
        format!("{}-{}", self.package(), self.pvr())
    }

    /// Return the revision.
    pub fn pr(&self) -> String {
        format!("r{}", self.revision().map(|r| r.as_str()).unwrap_or("0"))
    }

    /// Return the version.
    pub fn pv(&self) -> String {
        self.version.base().to_string()
    }

    /// Return the version and revision.
    pub fn pvr(&self) -> String {
        self.version.as_str().to_string()
    }

    /// Return the category and package.
    pub fn cpn(&self) -> String {
        format!("{}/{}", self.category, self.package)
    }

    /// Return the relative ebuild file path.
    pub(crate) fn relpath(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(format!("{}/{}/{}.ebuild", self.category(), self.package(), self.pf()))
    }
}

impl FromStr for Cpv {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        parse::cpv(s)
    }
}

/// Try converting a (category, package, version) string tuple into a Cpv.
impl TryFrom<(&str, &str, &str)> for Cpv {
    type Error = Error;

    fn try_from(vals: (&str, &str, &str)) -> Result<Self, Self::Error> {
        let (cat, pn, ver) = vals;
        parse::cpv(&format!("{cat}/{pn}-{ver}"))
    }
}

impl fmt::Display for Cpv {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}-{}", self.category, self.package, self.version.as_str())
    }
}

/// Determine if two Cpvs intersect.
impl Intersects<Cpv> for Cpv {
    fn intersects(&self, other: &Cpv) -> bool {
        self == other
    }
}

/// Determine if a Cpv intersects with a package dependency.
impl Intersects<Dep> for Cpv {
    fn intersects(&self, other: &Dep) -> bool {
        other.intersects(self)
    }
}

impl TryFrom<&str> for Cpv {
    type Error = Error;

    fn try_from(value: &str) -> crate::Result<Cpv> {
        Cpv::from_str(value)
    }
}

impl TryFrom<&Cpv> for Cpv {
    type Error = Error;

    fn try_from(value: &Cpv) -> crate::Result<Cpv> {
        Ok(value.clone())
    }
}
