use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use itertools::Itertools;
use strum::{AsRefStr, Display, EnumString};

use crate::eapi::{Eapi, EAPI_LATEST};
use crate::macros::bool_not_equal;
use crate::types::OrderedSet;
use crate::Error;

use super::version::{Operator, ParsedVersion, Revision, Version};
use super::{parse, Cpv};

#[repr(C)]
#[derive(
    AsRefStr, Display, EnumString, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum Blocker {
    #[default]
    NONE,
    #[strum(serialize = "!!")]
    Strong,
    #[strum(serialize = "!")]
    Weak,
}

#[repr(C)]
#[derive(
    AsRefStr, Display, EnumString, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
pub enum SlotOperator {
    #[default]
    NONE,
    #[strum(serialize = "=")]
    Equal,
    #[strum(serialize = "*")]
    Star,
}

#[repr(C)]
#[derive(EnumString, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[strum(serialize_all = "snake_case")]
pub enum OptionalDepField {
    Blocker,
    Version,
    Slot,
    Subslot,
    SlotOp,
    UseDeps,
    Repo,
}

/// Parsed package dep from borrowed input string.
#[derive(Debug, Default)]
pub(crate) struct ParsedDep<'a> {
    pub(crate) category: &'a str,
    pub(crate) package: &'a str,
    pub(crate) blocker: Option<Blocker>,
    pub(crate) version: Option<ParsedVersion<'a>>,
    pub(crate) version_str: Option<&'a str>,
    pub(crate) slot: Option<&'a str>,
    pub(crate) subslot: Option<&'a str>,
    pub(crate) slot_op: Option<SlotOperator>,
    pub(crate) use_deps: Option<Vec<&'a str>>,
    pub(crate) repo: Option<&'a str>,
}

impl ParsedDep<'_> {
    pub(crate) fn into_owned(self) -> crate::Result<Dep> {
        let version = match (self.version, self.version_str) {
            (Some(v), Some(vs)) => Some(v.into_owned(vs)?),
            _ => None,
        };

        Ok(Dep {
            category: self.category.to_string(),
            package: self.package.to_string(),
            blocker: self.blocker,
            version,
            slot: self.slot.map(|s| s.to_string()),
            subslot: self.subslot.map(|s| s.to_string()),
            slot_op: self.slot_op,
            use_deps: self.use_deps.map(|u| {
                // sort use deps by the first letter or number
                let mut set = OrderedSet::from_iter(u.iter().map(|s| s.to_string()));
                let f = |c: &char| c >= &'0';
                set.sort_by(|u1, u2| u1.chars().find(f).cmp(&u2.chars().find(f)));
                set
            }),
            repo: self.repo.map(|s| s.to_string()),
        })
    }
}

/// Package dependency.
#[derive(Debug, Clone)]
pub struct Dep {
    category: String,
    package: String,
    blocker: Option<Blocker>,
    version: Option<Version>,
    slot: Option<String>,
    subslot: Option<String>,
    slot_op: Option<SlotOperator>,
    use_deps: Option<OrderedSet<String>>,
    repo: Option<String>,
}

impl PartialEq for Dep {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Dep {}

impl Hash for Dep {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

/// Key type used for implementing various traits, e.g. Eq, Hash, etc.
type DepKey<'a> = (
    &'a str,                        // category
    &'a str,                        // package
    Option<&'a Version>,            // version
    Option<Blocker>,                // blocker
    Option<&'a str>,                // slot
    Option<&'a str>,                // subslot
    Option<SlotOperator>,           // slot operator
    Option<&'a OrderedSet<String>>, // use deps
    Option<&'a str>,                // repo
);

impl Dep {
    /// Create a new Dep from a given string.
    pub fn new<T>(s: &str, eapi: T) -> crate::Result<Self>
    where
        T: TryInto<&'static Eapi>,
        Error: From<<T as TryInto<&'static Eapi>>::Error>,
    {
        parse::dep(s, eapi.try_into()?)
    }

    /// Create a new unversioned Dep from a given string.
    pub fn new_cpn(s: &str) -> crate::Result<Self> {
        parse::cpn(s)
    }

    /// Potentially create a new Dep dropping the given fields if they exist.
    pub fn without(&self, fields: &[OptionalDepField]) -> crate::Result<Cow<'_, Self>> {
        let mut dep = Cow::Borrowed(self);
        use OptionalDepField::*;
        for field in fields {
            match field {
                Blocker => {
                    if self.blocker.is_some() {
                        dep.to_mut().blocker = None;
                    }
                }
                Version => {
                    if self.version.is_some() {
                        dep.to_mut().version = None;
                    }
                }
                Slot => {
                    if self.slot.is_some() {
                        dep.to_mut().slot = None;
                    }
                }
                Subslot => {
                    if self.subslot.is_some() {
                        dep.to_mut().subslot = None;
                    }
                }
                SlotOp => {
                    if self.slot_op.is_some() {
                        dep.to_mut().slot_op = None;
                    }
                }
                UseDeps => {
                    if self.use_deps.is_some() {
                        dep.to_mut().use_deps = None;
                    }
                }
                Repo => {
                    if self.repo.is_some() {
                        dep.to_mut().repo = None;
                    }
                }
            }
        }

        let d = dep.as_ref();
        match (d.slot(), d.subslot(), d.slot_op()) {
            (None, Some(_), None) | (None, Some(_), Some(_)) => {
                Err(Error::InvalidValue("invalid slot fields".to_string()))
            }
            _ => Ok(dep),
        }
    }

    /// Verify a string represents a valid package dependency.
    pub fn valid<T>(s: &str, eapi: T) -> crate::Result<()>
    where
        T: TryInto<&'static Eapi>,
        Error: From<<T as TryInto<&'static Eapi>>::Error>,
    {
        parse::dep_str(s, eapi.try_into()?)?;
        Ok(())
    }

    /// Return a package dependency's category.
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Return a package dependency's package.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Return a package dependency's blocker.
    pub fn blocker(&self) -> Option<Blocker> {
        self.blocker
    }

    /// Return a package dependency's USE flag dependencies.
    pub fn use_deps(&self) -> Option<&OrderedSet<String>> {
        self.use_deps.as_ref()
    }

    /// Return a package dependency's version.
    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    /// Return a package dependency's revision.
    pub fn revision(&self) -> Option<&Revision> {
        self.version.as_ref().and_then(|v| v.revision())
    }

    /// Return a package dependency's version operator.
    pub fn op(&self) -> Option<Operator> {
        self.version.as_ref().and_then(|v| v.op())
    }

    /// Return the package name and version.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1".
    pub fn p(&self) -> String {
        match &self.version {
            Some(ver) => format!("{}-{}", self.package(), ver.base()),
            None => self.package().to_string(),
        }
    }

    /// Return the package name, version, and revision.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1-r2".
    pub fn pf(&self) -> String {
        match &self.version {
            Some(_) => format!("{}-{}", self.package(), self.pvr()),
            None => self.package().to_string(),
        }
    }

    /// Return the package dependency's revision.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "r2".
    pub fn pr(&self) -> String {
        match &self.version {
            Some(ver) => format!("r{}", ver.revision().map(|r| r.as_str()).unwrap_or("0")),
            None => String::default(),
        }
    }

    /// Return the package dependency's version.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "1".
    pub fn pv(&self) -> String {
        match &self.version {
            Some(ver) => ver.base().to_string(),
            None => String::default(),
        }
    }

    /// Return the package dependency's version and revision.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "1-r2".
    pub fn pvr(&self) -> String {
        match &self.version {
            Some(ver) => ver.as_str().to_string(),
            None => String::default(),
        }
    }

    /// Return the package dependency's category and package.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg".
    pub fn cpn(&self) -> String {
        format!("{}/{}", self.category, self.package)
    }

    /// Return the package dependency's category, package, version, and revision.
    /// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg-1-r2".
    pub fn cpv(&self) -> String {
        match &self.version {
            Some(ver) => format!("{}/{}-{}", self.category, self.package, ver.as_str()),
            None => self.cpn(),
        }
    }

    /// Return a package dependency's slot.
    pub fn slot(&self) -> Option<&str> {
        self.slot.as_deref()
    }

    /// Return a package dependency's subslot.
    pub fn subslot(&self) -> Option<&str> {
        self.subslot.as_deref()
    }

    /// Return a package dependency's slot operator.
    pub fn slot_op(&self) -> Option<SlotOperator> {
        self.slot_op
    }

    /// Return a package dependency's repository.
    pub fn repo(&self) -> Option<&str> {
        self.repo.as_deref()
    }

    /// Return a key value used to implement various traits, e.g. Eq, Hash, etc.
    fn key(&self) -> DepKey {
        (
            self.category(),
            self.package(),
            self.version(),
            self.blocker(),
            self.slot(),
            self.subslot(),
            self.slot_op(),
            self.use_deps(),
            self.repo(),
        )
    }
}

impl fmt::Display for Dep {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // append blocker
        if let Some(blocker) = self.blocker {
            write!(f, "{blocker}")?;
        }

        // append version operator with cpv
        let cpv = self.cpv();
        use Operator::*;
        match self.version.as_ref().and_then(|v| v.op()) {
            None => write!(f, "{cpv}")?,
            Some(Less) => write!(f, "<{cpv}")?,
            Some(LessOrEqual) => write!(f, "<={cpv}")?,
            Some(Equal) => write!(f, "={cpv}")?,
            Some(EqualGlob) => write!(f, "={cpv}*")?,
            Some(Approximate) => write!(f, "~{cpv}")?,
            Some(GreaterOrEqual) => write!(f, ">={cpv}")?,
            Some(Greater) => write!(f, ">{cpv}")?,
            Some(NONE) => panic!("Operator::NONE is only valid as a C bindings fallback"),
        }

        // append slot data
        match (self.slot(), self.subslot(), self.slot_op()) {
            (Some(slot), Some(subslot), Some(op)) => write!(f, ":{slot}/{subslot}{op}")?,
            (Some(slot), Some(subslot), None) => write!(f, ":{slot}/{subslot}")?,
            (Some(slot), None, Some(op)) => write!(f, ":{slot}{op}")?,
            (Some(x), None, None) => write!(f, ":{x}")?,
            (None, None, Some(x)) => write!(f, ":{x}")?,
            _ => (),
        }

        // append use deps
        if let Some(x) = &self.use_deps {
            write!(f, "[{}]", x.iter().join(","))?;
        }

        // append repo
        if let Some(repo) = &self.repo {
            write!(f, "::{repo}")?;
        }

        Ok(())
    }
}

impl Ord for Dep {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl PartialOrd for Dep {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Dep {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        parse::dep(s, &EAPI_LATEST)
    }
}

/// Determine if two objects intersect.
pub trait Intersects<T> {
    fn intersects(&self, obj: &T) -> bool;
}

/// Determine if a package dependency intersects with a Cpv.
impl Intersects<Cpv> for Dep {
    fn intersects(&self, other: &Cpv) -> bool {
        bool_not_equal!(&self.category(), &other.category());
        bool_not_equal!(&self.package(), &other.package());

        match self.version() {
            Some(ver) => ver.intersects(other.version()),
            None => true,
        }
    }
}

/// Determine if two package dependencies intersect ignoring blockers.
impl Intersects<Dep> for Dep {
    fn intersects(&self, other: &Dep) -> bool {
        bool_not_equal!(&self.category(), &other.category());
        bool_not_equal!(&self.package(), &other.package());

        if let (Some(x), Some(y)) = (self.slot(), other.slot()) {
            bool_not_equal!(x, y);
        }

        if let (Some(x), Some(y)) = (self.subslot(), other.subslot()) {
            bool_not_equal!(x, y);
        }

        if let (Some(x), Some(y)) = (self.use_deps(), other.use_deps()) {
            let flags: HashSet<_> = x.symmetric_difference(y).map(|s| s.as_str()).collect();
            for f in &flags {
                if f.starts_with('-') && flags.contains(&f[1..]) {
                    return false;
                }
            }
        }

        if let (Some(x), Some(y)) = (self.repo(), other.repo()) {
            bool_not_equal!(x, y);
        }

        match (self.version(), other.version()) {
            (Some(x), Some(y)) => x.intersects(y),
            (None, _) | (_, None) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use crate::dep::CpvOrDep;
    use crate::test::TEST_DATA;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn test_to_string() {
        for s in [
            "cat/pkg",
            "<cat/pkg-4",
            "<=cat/pkg-4-r1",
            "=cat/pkg-4-r0",
            "=cat/pkg-4-r01",
            "=cat/pkg-4*",
            "~cat/pkg-4",
            ">=cat/pkg-r1-2-r3",
            ">cat/pkg-4-r1:0=",
            ">cat/pkg-4-r1:0/2=[use]",
            ">cat/pkg-4-r1:0/2=[use]::repo",
            "!cat/pkg",
            "!!<cat/pkg-4",
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.to_string(), s);
        }

        // Package dependencies with certain use flag patterns aren't returned 1 to 1 since use
        // flags are sorted into an ordered set for equivalency purposes.
        for (s, expected) in [("cat/pkg[u,u]", "cat/pkg[u]"), ("cat/pkg[b,a]", "cat/pkg[a,b]")] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.to_string(), expected);
        }
    }

    #[test]
    fn test_cpn() {
        for (s, key) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg"),
            ("<=cat/pkg-4-r1", "cat/pkg"),
            ("=cat/pkg-4", "cat/pkg"),
            ("=cat/pkg-4*", "cat/pkg"),
            ("~cat/pkg-4", "cat/pkg"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1"),
            (">cat/pkg-4-r1:0=", "cat/pkg"),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.cpn(), key);
        }
    }

    #[test]
    fn test_version() {
        for (s, version) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", Some("<4")),
            ("<=cat/pkg-4-r1", Some("<=4-r1")),
            ("=cat/pkg-4", Some("=4")),
            ("=cat/pkg-4*", Some("=4*")),
            ("~cat/pkg-4", Some("~4")),
            (">=cat/pkg-r1-2-r3", Some(">=2-r3")),
            (">cat/pkg-4-r1:0=", Some(">4-r1")),
        ] {
            let dep: Dep = s.parse().unwrap();
            let version = version.map(|s| parse::version_with_op(s).unwrap());
            assert_eq!(dep.version(), version.as_ref());
        }
    }

    #[test]
    fn test_revision() {
        for (s, rev_str) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", None),
            ("=cat/pkg-4-r0", Some("0")),
            ("<=cat/pkg-4-r1", Some("1")),
            (">=cat/pkg-r1-2-r3", Some("3")),
            (">cat/pkg-4-r1:0=", Some("1")),
        ] {
            let dep: Dep = s.parse().unwrap();
            let rev = rev_str.map(|s| s.parse().unwrap());
            assert_eq!(dep.revision(), rev.as_ref(), "{s} failed");
        }
    }

    #[test]
    fn test_op() {
        for (s, op) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", Some(Operator::Less)),
            ("<=cat/pkg-4-r1", Some(Operator::LessOrEqual)),
            ("=cat/pkg-4", Some(Operator::Equal)),
            ("=cat/pkg-4*", Some(Operator::EqualGlob)),
            ("~cat/pkg-4", Some(Operator::Approximate)),
            (">=cat/pkg-r1-2-r3", Some(Operator::GreaterOrEqual)),
            (">cat/pkg-4-r1:0=", Some(Operator::Greater)),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.op(), op);
        }
    }

    #[test]
    fn test_cpv() {
        for (s, cpv) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg-4"),
            ("<=cat/pkg-4-r1", "cat/pkg-4-r1"),
            ("=cat/pkg-4", "cat/pkg-4"),
            ("=cat/pkg-4*", "cat/pkg-4"),
            ("~cat/pkg-4", "cat/pkg-4"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1-2-r3"),
            (">cat/pkg-4-r1:0=", "cat/pkg-4-r1"),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(dep.cpv(), cpv);
        }
    }

    #[test]
    fn test_cmp() {
        let op_map: HashMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        for (expr, (s1, op, s2)) in TEST_DATA.dep_toml.compares() {
            let dep1: Dep = s1.parse().unwrap();
            let dep2: Dep = s2.parse().unwrap();
            if op == "!=" {
                assert_ne!(dep1, dep2, "failed comparing {expr}");
                assert_ne!(dep2, dep1, "failed comparing {expr}");
            } else {
                let op = op_map[op];
                assert_eq!(dep1.cmp(&dep2), op, "failed comparing {expr}");
                assert_eq!(dep2.cmp(&dep1), op.reverse(), "failed comparing {expr}");

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(dep1), hash(dep2), "failed hash {expr}");
                }
            }
        }
    }

    #[test]
    fn test_intersects() {
        // inject version intersects data from version.toml into Dep objects
        let dep = Dep::from_str("a/b").unwrap();
        for d in &TEST_DATA.version_toml.intersects {
            // test intersections between all pairs of distinct values
            let permutations = d
                .vals
                .iter()
                .map(|s| s.as_str())
                .permutations(2)
                .map(|val| val.into_iter().collect_tuple().unwrap());
            for (s1, s2) in permutations {
                let (mut dep1, mut dep2) = (dep.clone(), dep.clone());
                dep1.version = Some(s1.parse().unwrap());
                dep2.version = Some(s2.parse().unwrap());

                // self intersection
                assert!(dep1.intersects(&dep1), "{dep1} doesn't intersect itself");
                assert!(dep2.intersects(&dep2), "{dep2} doesn't intersect itself");

                // intersects depending on status
                if d.status {
                    assert!(dep1.intersects(&dep2), "{dep1} doesn't intersect {dep2}");
                } else {
                    assert!(!dep1.intersects(&dep2), "{dep1} intersects {dep2}");
                }
            }
        }

        for d in &TEST_DATA.dep_toml.intersects {
            // test intersections between all pairs of distinct values
            let permutations = d
                .vals
                .iter()
                .map(|s| s.as_str())
                .permutations(2)
                .map(|val| val.into_iter().collect_tuple().unwrap());
            for (s1, s2) in permutations {
                let obj1: CpvOrDep = s1.parse().unwrap();
                let obj2: CpvOrDep = s2.parse().unwrap();

                // self intersection
                assert!(obj1.intersects(&obj1), "{obj1} doesn't intersect {obj1}");
                assert!(obj2.intersects(&obj2), "{obj2} doesn't intersect {obj2}");

                // intersects depending on status
                if d.status {
                    assert!(obj1.intersects(&obj2), "{obj1} doesn't intersect {obj2}");
                } else {
                    assert!(!obj1.intersects(&obj2), "{obj1} intersects {obj2}");
                }
            }
        }
    }

    #[test]
    fn test_sorting() {
        for d in &TEST_DATA.dep_toml.sorting {
            let mut reversed: Vec<Dep> =
                d.sorted.iter().map(|s| s.parse().unwrap()).rev().collect();
            reversed.sort();
            let mut sorted: Vec<_> = reversed.iter().map(|x| x.to_string()).collect();
            if d.equal {
                // equal deps aren't sorted so reversing should restore the original order
                sorted = sorted.into_iter().rev().collect();
            }
            assert_eq!(&sorted, &d.sorted);
        }
    }

    #[test]
    fn test_hashing() {
        for d in &TEST_DATA.version_toml.hashing {
            let set: HashSet<Dep> = d
                .versions
                .iter()
                .map(|s| format!("=cat/pkg-{s}").parse().unwrap())
                .collect();
            if d.equal {
                assert_eq!(set.len(), 1, "failed hashing deps: {set:?}");
            } else {
                assert_eq!(set.len(), d.versions.len(), "failed hashing deps: {set:?}");
            }
        }
    }
}
