use std::borrow::Cow;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use camino::Utf8PathBuf;
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::traits::Intersects;
use crate::Error;

use super::cpn::Cpn;
use super::parse;
use super::pkg::Dep;
use super::version::{Operator, Revision, Version, WithOp};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum CpvOrDep {
    Cpv(Cpv),
    Dep(Dep),
}

impl CpvOrDep {
    /// Create a [`CpvOrDep`] from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        parse::cpv_or_dep(s)
    }
}

impl FromStr for CpvOrDep {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
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

impl Intersects for CpvOrDep {
    fn intersects(&self, other: &Self) -> bool {
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

impl Intersects<CpvOrDep> for Cpv {
    fn intersects(&self, other: &CpvOrDep) -> bool {
        other.intersects(self)
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

impl Intersects<CpvOrDep> for Dep {
    fn intersects(&self, other: &CpvOrDep) -> bool {
        other.intersects(self)
    }
}

impl Intersects<Cow<'_, Dep>> for CpvOrDep {
    fn intersects(&self, other: &Cow<'_, Dep>) -> bool {
        use CpvOrDep::*;
        match (self, other) {
            (Cpv(obj1), obj2) => obj1.intersects(obj2),
            (Dep(obj1), obj2) => obj1.intersects(obj2),
        }
    }
}

impl Intersects<CpvOrDep> for Cow<'_, Dep> {
    fn intersects(&self, other: &CpvOrDep) -> bool {
        other.intersects(self)
    }
}

/// Versioned package.
#[derive(SerializeDisplay, DeserializeFromStr, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct Cpv {
    pub(crate) cpn: Cpn,
    pub(crate) version: Version,
}

impl WithOp for Cpv {
    type WithOp = Dep;

    fn with_op(self, op: Operator) -> Result<Self::WithOp, &'static str> {
        Ok(Dep {
            cpn: self.cpn,
            version: Some(self.version.with_op(op)?),
            blocker: None,
            slot_dep: None,
            use_deps: None,
            repo: None,
        })
    }
}

impl Cpv {
    /// Create a [`Cpv`] from a given string.
    pub fn try_new<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        parse::cpv(s.as_ref())
    }

    /// Return the [`Cpn`].
    pub fn cpn(&self) -> &Cpn {
        &self.cpn
    }

    /// Return a Cpv's category.
    pub fn category(&self) -> &str {
        self.cpn.category()
    }

    /// Return a Cpv's package.
    pub fn package(&self) -> &str {
        self.cpn.package()
    }

    /// Return a Cpv's version.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Return a Cpv's revision.
    pub fn revision(&self) -> Option<&Revision> {
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

    /// Return the relative ebuild file path.
    pub(crate) fn relpath(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(format!("{}/{}.ebuild", self.cpn(), self.pf()))
    }
}

impl FromStr for Cpv {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

/// Try converting a (category, package, version) string tuple into a Cpv.
impl<T1, T2, T3> TryFrom<(T1, T2, T3)> for Cpv
where
    T1: fmt::Display,
    T2: fmt::Display,
    T3: fmt::Display,
{
    type Error = Error;

    fn try_from(vals: (T1, T2, T3)) -> Result<Self, Self::Error> {
        let (cat, pn, ver) = vals;
        Cpv::try_new(format!("{cat}/{pn}-{ver}"))
    }
}

impl fmt::Debug for Cpv {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cpv {{ {self} }}")
    }
}

impl fmt::Display for Cpv {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}", self.cpn, self.version)
    }
}

/// Determine if two Cpvs intersect.
impl Intersects for Cpv {
    fn intersects(&self, other: &Self) -> bool {
        self == other
    }
}

impl TryFrom<&str> for Cpv {
    type Error = Error;

    fn try_from(value: &str) -> crate::Result<Self> {
        Cpv::try_new(value)
    }
}

impl From<&Cpv> for Cpv {
    fn from(value: &Cpv) -> Self {
        value.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::collections::HashMap;

    use crate::test::TEST_DATA;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn new_and_parse() {
        // invalid
        for s in ["a/b", "a/b-1a-1", "a/b1", "a/b-1aa", "a/b-1.a", "a/b-1-r2-3-r4"] {
            assert!(Cpv::try_new(s).is_err(), "{s} is valid");
        }

        // valid
        for s in ["a/b--1", "a/b-r1-2", "a/b-r0-1-r2", "a/b-3-c-4-r5"] {
            let cpv = Cpv::try_new(s);
            assert!(cpv.is_ok(), "{s} isn't valid");
        }
    }

    #[test]
    fn cmp() {
        let op_map: HashMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        for (expr, (s1, op, s2)) in TEST_DATA.version_toml.compares() {
            let cpv_str1 = format!("a/b-{s1}");
            let cpv_str2 = format!("a/b-{s2}");
            let cpv1 = Cpv::try_new(&cpv_str1).unwrap();
            let cpv2 = Cpv::try_new(&cpv_str2).unwrap();
            if op == "!=" {
                assert_ne!(cpv1, cpv2, "failed comparing: {expr}");
                assert_ne!(cpv2, cpv1, "failed comparing: {expr}");
            } else {
                let op = op_map[op];
                assert_eq!(cpv1.cmp(&cpv2), op, "failed comparing: {expr}");
                assert_eq!(cpv2.cmp(&cpv1), op.reverse(), "failed comparing inverted: {expr}");

                // package and category names take priority over versions for comparisons
                let cpv_str3 = format!("a/z-{s2}");
                let cpv_str4 = format!("z/b-{s2}");
                let cpv3 = Cpv::try_new(&cpv_str3).unwrap();
                let cpv4 = Cpv::try_new(&cpv_str4).unwrap();
                assert!(cpv3 > cpv1);
                assert!(cpv3 > cpv2);
                assert!(cpv4 > cpv3);

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(&cpv1), hash(&cpv2), "failed hash: {expr}");
                }
            }
        }
    }

    #[test]
    fn cpv_or_dep() {
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        let dep = Dep::try_new(">=cat/pkg-1").unwrap();
        let dep_cow = dep.without([]).unwrap();

        // valid
        for s in ["cat/pkg", "cat/pkg-1", ">=cat/pkg-1"] {
            let cpv_or_dep = CpvOrDep::try_new(s).unwrap();
            assert_eq!(cpv_or_dep.to_string(), s);
            assert_eq!(cpv_or_dep, s.parse().unwrap());

            // intersects with itself
            assert!(cpv_or_dep.intersects(&cpv_or_dep));

            // intersects with Cpv
            assert!(cpv_or_dep.intersects(&cpv));
            assert!(cpv.intersects(&cpv_or_dep));

            // intersects with Dep
            assert!(cpv_or_dep.intersects(&dep));
            assert!(cpv_or_dep.intersects(&dep_cow));
            assert!(dep.intersects(&cpv_or_dep));
            assert!(dep_cow.intersects(&cpv_or_dep));
        }

        // variants intersect
        let cpv_or_dep1 = CpvOrDep::try_new("cat/pkg-1").unwrap();
        let cpv_or_dep2 = CpvOrDep::try_new(">=cat/pkg-1").unwrap();
        assert!(cpv_or_dep1.intersects(&cpv_or_dep2));
        assert!(cpv_or_dep2.intersects(&cpv_or_dep1));

        // invalid
        assert!(CpvOrDep::try_new("cat/pkg-1a-1").is_err());
        assert!(CpvOrDep::try_new("cat").is_err());
    }
}
