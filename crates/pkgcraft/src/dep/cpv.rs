use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::macros::{equivalent, partial_cmp_not_equal_opt};
use crate::traits::{Intersects, IntoOwned, ToRef};
use crate::Error;

use super::pkg::Dep;
use super::version::{Operator, Revision, Version, WithOp};
use super::{parse, Stringable};

pub enum CpvOrDep<S: Stringable> {
    Cpv(Cpv<S>),
    Dep(Dep<S>),
}

impl<'a> IntoOwned for CpvOrDep<&'a str> {
    type Owned = CpvOrDep<String>;

    fn into_owned(self) -> Self::Owned {
        match self {
            CpvOrDep::Cpv(val) => CpvOrDep::Cpv(val.into_owned()),
            CpvOrDep::Dep(val) => CpvOrDep::Dep(val.into_owned()),
        }
    }
}

impl CpvOrDep<String> {
    /// Create an owned [`CpvOrDep`] from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        CpvOrDep::parse(s).into_owned()
    }
}

impl<'a> CpvOrDep<&'a str> {
    /// Create a borrowed [`CpvOrDep`] from a given string.
    pub fn parse(s: &'a str) -> crate::Result<Self> {
        if let Ok(val) = Dep::parse(s, None) {
            Ok(CpvOrDep::Dep(val))
        } else if let Ok(val) = Cpv::parse(s) {
            Ok(CpvOrDep::Cpv(val))
        } else {
            Err(Error::InvalidValue(format!("invalid cpv or dep: {s}")))
        }
    }
}

impl FromStr for CpvOrDep<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

impl<S: Stringable> fmt::Display for CpvOrDep<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Cpv(val) => write!(f, "{val}"),
            Self::Dep(val) => write!(f, "{val}"),
        }
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<CpvOrDep<S1>> for CpvOrDep<S2> {
    fn intersects(&self, other: &CpvOrDep<S1>) -> bool {
        use CpvOrDep::*;
        match (self, other) {
            (Cpv(obj1), Cpv(obj2)) => obj1.intersects(obj2),
            (Cpv(obj1), Dep(obj2)) => obj1.intersects(obj2),
            (Dep(obj1), Cpv(obj2)) => obj1.intersects(obj2),
            (Dep(obj1), Dep(obj2)) => obj1.intersects(obj2),
        }
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<Cpv<S1>> for CpvOrDep<S2> {
    fn intersects(&self, other: &Cpv<S1>) -> bool {
        use CpvOrDep::*;
        match (self, other) {
            (Cpv(obj1), obj2) => obj1.intersects(obj2),
            (Dep(obj1), obj2) => obj1.intersects(obj2),
        }
    }
}

impl<S1: Stringable, S2: Stringable> Intersects<Dep<S1>> for CpvOrDep<S2> {
    fn intersects(&self, other: &Dep<S1>) -> bool {
        use CpvOrDep::*;
        match (self, other) {
            (Cpv(obj1), obj2) => obj1.intersects(obj2),
            (Dep(obj1), obj2) => obj1.intersects(obj2),
        }
    }
}

/// Package identifier.
#[derive(Debug, Serialize, Deserialize, Eq, Ord, Clone, Hash)]
pub struct Cpv<S: Stringable> {
    pub(crate) category: S,
    pub(crate) package: S,
    pub(crate) version: Version<S>,
}

impl<'a> WithOp for Cpv<&'a str> {
    type WithOp = Dep<&'a str>;

    fn with_op(self, op: Operator) -> Result<Self::WithOp, &'static str> {
        Ok(Dep {
            category: self.category,
            package: self.package,
            version: Some(self.version.with_op(op)?),
            ..Default::default()
        })
    }
}

impl<'a> IntoOwned for Cpv<&'a str> {
    type Owned = Cpv<String>;

    fn into_owned(self) -> Self::Owned {
        Cpv {
            category: self.category.to_string(),
            package: self.package.to_string(),
            version: self.version.into_owned(),
        }
    }
}

impl<'a> ToRef<'a> for Cpv<String> {
    type Ref = Cpv<&'a str>;

    fn to_ref(&'a self) -> Self::Ref {
        Cpv {
            category: self.category.as_ref(),
            package: self.package.as_ref(),
            version: self.version.to_ref(),
        }
    }
}

impl Cpv<String> {
    /// Create an owned [`Cpv`] from a given string.
    pub fn try_new<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        Cpv::parse(s.as_ref()).into_owned()
    }
}

impl<'a> Cpv<&'a str> {
    /// Create a borrowed [`Cpv`] from a given string.
    pub fn parse(s: &'a str) -> crate::Result<Self> {
        parse::cpv(s)
    }
}

impl<S: Stringable> Cpv<S> {
    /// Return a Cpv's category.
    pub fn category(&self) -> &str {
        self.category.as_ref()
    }

    /// Return a Cpv's package.
    pub fn package(&self) -> &str {
        self.package.as_ref()
    }

    /// Return a Cpv's version.
    pub fn version(&self) -> &Version<S> {
        &self.version
    }

    /// Return a Cpv's revision.
    pub fn revision(&self) -> Option<&Revision<S>> {
        self.version.revision()
    }

    /// Return the string of the package name and version without the revision.
    pub fn p(&self) -> String {
        format!("{}-{}", self.package(), self.version.base())
    }

    /// Return the string of the package name and version with the revision.
    pub fn pf(&self) -> String {
        format!("{}-{}", self.package(), self.pvr())
    }

    /// Return the string of the revision.
    pub fn pr(&self) -> String {
        format!("r{}", self.revision().map(|r| r.as_str()).unwrap_or("0"))
    }

    /// Return the string of the version without the revision.
    pub fn pv(&self) -> String {
        self.version.base()
    }

    /// Return the string of the version with the revision.
    pub fn pvr(&self) -> String {
        self.version.to_string()
    }

    /// Return the string of the category and package name.
    pub fn cpn(&self) -> String {
        format!("{}/{}", self.category, self.package)
    }

    /// Return the relative ebuild file path.
    pub(crate) fn relpath(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(format!("{}/{}/{}.ebuild", self.category(), self.package(), self.pf()))
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Cpv<S1>> for Cpv<S2> {
    fn eq(&self, other: &Cpv<S1>) -> bool {
        self.category() == other.category()
            && self.package() == other.package()
            && self.version == other.version
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Cpv<S1>> for Cpv<S2> {
    fn partial_cmp(&self, other: &Cpv<S1>) -> Option<Ordering> {
        partial_cmp_not_equal_opt!(self.category(), other.category());
        partial_cmp_not_equal_opt!(self.package(), other.package());
        self.version.partial_cmp(&other.version)
    }
}

impl FromStr for Cpv<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

/// Try converting a (category, package, version) string tuple into a Cpv.
impl TryFrom<(&str, &str, &str)> for Cpv<String> {
    type Error = Error;

    fn try_from(vals: (&str, &str, &str)) -> Result<Self, Self::Error> {
        let (cat, pn, ver) = vals;
        Cpv::try_new(format!("{cat}/{pn}-{ver}"))
    }
}

impl<S: Stringable> fmt::Display for Cpv<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}-{}", self.category, self.package, self.version)
    }
}

/// Determine if two Cpvs intersect.
impl<S1: Stringable, S2: Stringable> Intersects<Cpv<S1>> for Cpv<S2> {
    fn intersects(&self, other: &Cpv<S1>) -> bool {
        self == other
    }
}

/// Determine if a Cpv intersects with a package dependency.
impl<S1: Stringable, S2: Stringable> Intersects<Dep<S1>> for Cpv<S2> {
    fn intersects(&self, other: &Dep<S1>) -> bool {
        other.intersects(self)
    }
}

impl TryFrom<&str> for Cpv<String> {
    type Error = Error;

    fn try_from(value: &str) -> crate::Result<Cpv<String>> {
        value.parse()
    }
}

impl TryFrom<&Cpv<String>> for Cpv<String> {
    type Error = Error;

    fn try_from(value: &Cpv<String>) -> crate::Result<Cpv<String>> {
        Ok(value.clone())
    }
}

equivalent!(Cpv);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        assert!(Cpv::try_new("cat/pkg-1").is_ok());
        assert!(Cpv::try_new("cat/pkg-1a-1").is_err());
        assert!(Cpv::try_new("cat/pkg").is_err());
        assert!(Cpv::try_new(">=cat/pkg-1").is_err());
    }

    #[test]
    fn test_parse() {
        assert!(Cpv::parse("cat/pkg-1").is_ok());
        assert!(Cpv::parse("cat/pkg-1a-1").is_err());
        assert!(Cpv::parse("cat/pkg").is_err());
        assert!(Cpv::parse(">=cat/pkg-1").is_err());
    }

    #[test]
    fn test_cpv_or_dep() {
        let cpv = Cpv::from_str("cat/pkg-1").unwrap();
        let dep = Dep::from_str(">=cat/pkg-1").unwrap();

        // valid
        for s in ["cat/pkg", "cat/pkg-1", ">=cat/pkg-1"] {
            let cpv_or_dep: CpvOrDep<_> = s.parse().unwrap();
            assert_eq!(cpv_or_dep.to_string(), s);
            // intersects
            assert!(cpv_or_dep.intersects(&cpv));
            assert!(cpv_or_dep.intersects(&dep));
            assert!(cpv_or_dep.intersects(&cpv_or_dep));
        }

        // invalid
        assert!(CpvOrDep::from_str("cat/pkg-1a-1").is_err());
        assert!(CpvOrDep::from_str("cat").is_err());
    }
}
