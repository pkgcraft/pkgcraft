use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::iter::zip;
use std::str::FromStr;
use std::{fmt, str};

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

use crate::macros::cmp_not_equal;
use crate::Error;

use super::{parse, Intersects};

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Suffix {
    Alpha(Option<u64>),
    Beta(Option<u64>),
    Pre(Option<u64>),
    Rc(Option<u64>),
    P(Option<u64>),
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Revision {
    value: Option<String>,
    int: u64,
}

impl FromStr for Revision {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        if s.is_empty() {
            Ok(Revision::default())
        } else {
            let int = s
                .parse()
                .map_err(|e| Error::Overflow(format!("invalid revision: {e}: {s}")))?;
            Ok(Revision {
                value: Some(s.to_string()),
                int,
            })
        }
    }
}

impl Revision {
    pub fn as_str(&self) -> &str {
        self.value.as_deref().unwrap_or_default()
    }
}

impl PartialEq for Revision {
    fn eq(&self, other: &Self) -> bool {
        self.int == other.int
    }
}

impl Eq for Revision {}

impl Hash for Revision {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.int.hash(state);
    }
}

impl PartialEq<str> for Revision {
    fn eq(&self, other: &str) -> bool {
        match &self.value {
            Some(s) => s == other,
            None => "0" == other,
        }
    }
}

impl Ord for Revision {
    fn cmp(&self, other: &Self) -> Ordering {
        self.int.cmp(&other.int)
    }
}

impl PartialOrd for Revision {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.int.partial_cmp(&other.int)
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&Revision> for String {
    fn from(rev: &Revision) -> Self {
        rev.as_str().into()
    }
}

#[derive(Debug)]
pub(crate) struct ParsedVersion<'a> {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) base_end: usize,
    pub(crate) op: Option<Operator>,
    pub(crate) numbers: Vec<&'a str>,
    pub(crate) letter: Option<char>,
    pub(crate) suffixes: Vec<Suffix>,
    pub(crate) revision: Option<&'a str>,
}

impl<'a> ParsedVersion<'a> {
    // Used by the parser to inject the version operator value.
    pub(crate) fn with_op(
        mut self,
        op: &'a str,
        glob: Option<&'a str>,
    ) -> Result<Self, &'static str> {
        use Operator::*;
        let op = match (op, glob) {
            ("<", None) => Ok(Less),
            ("<=", None) => Ok(LessOrEqual),
            ("=", None) => Ok(Equal),
            ("=", Some(_)) => Ok(EqualGlob),
            ("~", None) => match self.revision {
                None => Ok(Approximate),
                Some(_) => Err("~ version operator can't be used with a revision"),
            },
            (">=", None) => Ok(GreaterOrEqual),
            (">", None) => Ok(Greater),
            _ => Err("invalid version operator"),
        }?;

        self.op = Some(op);
        Ok(self)
    }

    pub(crate) fn into_owned(self, input: &str) -> crate::Result<Version> {
        let mut numbers = Vec::<(String, u64)>::new();
        for s in self.numbers.iter() {
            let num = s
                .parse()
                .map_err(|e| Error::Overflow(format!("invalid version: {e}: {s}")))?;
            numbers.push((s.to_string(), num));
        }

        Ok(Version {
            full: input[self.start..self.end].to_string(),
            base_end: self.base_end,
            op: self.op,
            numbers,
            letter: self.letter,
            suffixes: self.suffixes,
            revision: Revision::from_str(self.revision.unwrap_or_default())?,
        })
    }
}

#[repr(C)]
#[derive(
    AsRefStr,
    Display,
    EnumString,
    Debug,
    Default,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
)]
pub enum Operator {
    #[default]
    NONE,
    #[strum(serialize = "<")]
    Less,
    #[strum(serialize = "<=")]
    LessOrEqual,
    #[strum(serialize = "=")]
    Equal,
    #[strum(serialize = "=*")]
    EqualGlob,
    #[strum(serialize = "~")]
    Approximate,
    #[strum(serialize = ">=")]
    GreaterOrEqual,
    #[strum(serialize = ">")]
    Greater,
}

impl Operator {
    fn intersects(&self, left: &Version, right: &Version) -> bool {
        use Operator::*;
        match self {
            Less => NonOpVersion(right) < NonOpVersion(left),
            LessOrEqual => NonOpVersion(right) <= NonOpVersion(left),
            Equal => NonOpVersion(right) == NonOpVersion(left),
            EqualGlob => right.as_str().starts_with(left.as_str()),
            Approximate => NonRevisionVersion(right) == NonRevisionVersion(left),
            GreaterOrEqual => NonOpVersion(right) >= NonOpVersion(left),
            Greater => NonOpVersion(right) > NonOpVersion(left),
            NONE => panic!("Operator::NONE is only valid as a C bindings fallback"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Version {
    full: String,
    base_end: usize,
    op: Option<Operator>,
    numbers: Vec<(String, u64)>,
    letter: Option<char>,
    suffixes: Vec<Suffix>,
    revision: Revision,
}

impl Version {
    /// Verify a string represents a valid version.
    pub fn valid(s: &str) -> crate::Result<()> {
        parse::version_str(s)?;
        Ok(())
    }

    /// Create a new Version from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        parse::version(s).or_else(|e| match e {
            Error::Overflow(_) => Err(e),
            _ => parse::version_with_op(s),
        })
    }

    /// Return a version's string value without operator.
    pub fn as_str(&self) -> &str {
        &self.full
    }

    /// Return a version's operator, if one exists.
    pub fn op(&self) -> Option<Operator> {
        self.op
    }

    /// Return a version's base -- all components except the revision.
    pub fn base(&self) -> &str {
        &self.full[0..self.base_end]
    }

    /// Return a version's revision.
    pub fn revision(&self) -> Option<&Revision> {
        self.revision.value.as_ref().map(|_| &self.revision)
    }
}

/// Determine if two versions intersect.
impl Intersects<Version> for Version {
    fn intersects(&self, other: &Version) -> bool {
        use Operator::*;
        match (self.op(), other.op()) {
            // intersects if both are unbounded in the same direction
            (Some(Less), Some(Less)) | (Some(LessOrEqual), Some(LessOrEqual)) => true,
            (Some(Less), Some(LessOrEqual)) | (Some(LessOrEqual), Some(Less)) => true,
            (Some(Greater), Some(Greater)) | (Some(GreaterOrEqual), Some(GreaterOrEqual)) => true,
            (Some(Greater), Some(GreaterOrEqual)) | (Some(GreaterOrEqual), Some(Greater)) => true,

            // both non-op or '~' -- intersects if equal
            (None, None) | (Some(Approximate), Some(Approximate)) => self == other,

            // either non-op or '=' -- intersects if the other matches it
            (Some(op), None) | (Some(op), Some(Equal)) => op.intersects(self, other),
            (None, Some(op)) | (Some(Equal), Some(op)) => op.intersects(other, self),

            // both '=*' -- intersects if either glob matches
            (Some(op @ EqualGlob), Some(EqualGlob)) => {
                op.intersects(self, other) || op.intersects(other, self)
            }

            // '=*' and '~' -- intersects if glob matches unrevisioned version
            (Some(EqualGlob), Some(Approximate)) => other.as_str().starts_with(self.base()),
            (Some(Approximate), Some(EqualGlob)) => self.as_str().starts_with(other.base()),

            (Some(left_op), Some(right_op)) => {
                // remaining cases must have one op that is unbounded
                let (ranged, ranged_op, other, other_op) = match left_op {
                    Less | LessOrEqual | Greater | GreaterOrEqual => {
                        (self, left_op, other, right_op)
                    }
                    _ => (other, right_op, self, left_op),
                };

                match other_op {
                    // unbounded in opposite directions -- intersects if both match
                    Less | LessOrEqual | Greater | GreaterOrEqual => {
                        other_op.intersects(other, ranged) && ranged_op.intersects(ranged, other)
                    }

                    // '~' or '=*' -- intersects if range matches
                    Approximate | EqualGlob if ranged_op.intersects(ranged, other) => true,

                    // remaining '~' -- intersects if ranged is '>' or '>=' on other's version with
                    // a nonzero revision, e.g. >1-r1 intersects with ~1
                    Approximate => {
                        let greater = matches!(ranged_op, Greater | GreaterOrEqual);
                        greater && other_op.intersects(other, ranged)
                    }

                    // '=*' and '<' or '<=' -- intersects if the other revision is 0 or doesn't
                    // exist and glob matches ranged version
                    EqualGlob if matches!(ranged_op, Less | LessOrEqual) => {
                        match other.revision().map(|r| r.as_str()) {
                            None | Some("0") => ranged.as_str().starts_with(other.as_str()),
                            _ => false,
                        }
                    }

                    // remaining '=*' -- intersects if glob matches ranged version
                    EqualGlob => ranged.as_str().starts_with(other.as_str()),

                    // remaining variants that should never occur
                    Equal => unreachable!("Operator::Equal should be previously handled"),
                    NONE => panic!("Operator::NONE is only valid as a C bindings fallback"),
                }
            }
        }
    }
}

impl AsRef<Version> for Version {
    fn as_ref(&self) -> &Version {
        self
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Operator::*;

        let s = self.as_str();
        match self.op() {
            None => write!(f, "{}", s),
            Some(Less) => write!(f, "<{}", s),
            Some(LessOrEqual) => write!(f, "<={}", s),
            Some(Equal) => write!(f, "={}", s),
            Some(EqualGlob) => write!(f, "={}*", s),
            Some(Approximate) => write!(f, "~{}", s),
            Some(GreaterOrEqual) => write!(f, ">={}", s),
            Some(Greater) => write!(f, ">{}", s),
            Some(NONE) => panic!("Operator::NONE is only valid as a C bindings fallback"),
        }
    }
}

impl From<&Version> for String {
    fn from(ver: &Version) -> Self {
        ver.as_str().into()
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Version {}

impl Hash for Version {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.numbers[0].1.hash(state);
        for (v1, n1) in &self.numbers[1..] {
            if v1.starts_with('0') {
                v1.trim_end_matches('0').hash(state);
            } else {
                n1.hash(state);
            }
        }
        self.letter.hash(state);
        self.suffixes.hash(state);
        self.revision.hash(state);
    }
}

fn ver_cmp<V: AsRef<Version>>(v1: V, v2: V, cmp_revs: bool, cmp_ops: bool) -> Ordering {
    let (v1, v2) = (v1.as_ref(), v2.as_ref());

    if v1.base() != v2.base() {
        // compare major versions
        cmp_not_equal!(&v1.numbers[0].1, &v2.numbers[0].1);

        // iterate through the remaining version components
        for ((s1, n1), (s2, n2)) in zip(&v1.numbers[1..], &v2.numbers[1..]) {
            // if string is lexically equal, it is numerically equal too
            if s1 == s2 {
                continue;
            }

            // If one of the components starts with a "0" then they are compared as strings
            // with trailing 0's stripped, otherwise they are compared as integers.
            if s1.starts_with('0') || s2.starts_with('0') {
                cmp_not_equal!(s1.trim_end_matches('0'), s2.trim_end_matches('0'));
            } else {
                cmp_not_equal!(&n1, &n2);
            }
        }

        // compare the number of version components
        cmp_not_equal!(&v1.numbers.len(), &v2.numbers.len());

        // dotted components were equal so compare letter suffixes
        cmp_not_equal!(&v1.letter, &v2.letter);

        for (s1, s2) in zip(&v1.suffixes, &v2.suffixes) {
            // if suffixes differ, use them for comparison
            cmp_not_equal!(s1, s2);
        }

        // If one version has more suffixes, use the last suffix to determine ordering.
        match v1.suffixes.cmp(&v2.suffixes) {
            Ordering::Equal => (),
            Ordering::Greater => match v1.suffixes.last() {
                Some(Suffix::P(_)) => return Ordering::Greater,
                _ => return Ordering::Less,
            },
            Ordering::Less => match v2.suffixes.last() {
                Some(Suffix::P(_)) => return Ordering::Less,
                _ => return Ordering::Greater,
            },
        }
    }

    // compare the revisions
    if cmp_revs {
        cmp_not_equal!(&v1.revision, &v2.revision);
    }

    // compare the operators
    if cmp_ops {
        cmp_not_equal!(&v1.op, &v2.op);
    }

    Ordering::Equal
}

impl Ord for Version {
    fn cmp<'a>(&'a self, other: &'a Self) -> Ordering {
        ver_cmp(self, other, true, true)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::new(s)
    }
}

/// Version wrapper that ignores revisions and operators during comparisons.
#[derive(Debug, Eq, Hash, Clone)]
struct NonRevisionVersion<'a>(&'a Version);

impl AsRef<Version> for NonRevisionVersion<'_> {
    fn as_ref(&self) -> &Version {
        self.0
    }
}

impl PartialEq for NonRevisionVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Ord for NonRevisionVersion<'_> {
    fn cmp<'a>(&'a self, other: &'a Self) -> Ordering {
        ver_cmp(self, other, false, false)
    }
}

impl PartialOrd for NonRevisionVersion<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Version wrapper that ignores operators during comparisons.
#[derive(Debug, Eq, Hash, Clone)]
struct NonOpVersion<'a>(&'a Version);

impl AsRef<Version> for NonOpVersion<'_> {
    fn as_ref(&self) -> &Version {
        self.0
    }
}

impl PartialEq for NonOpVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Ord for NonOpVersion<'_> {
    fn cmp<'a>(&'a self, other: &'a Self) -> Ordering {
        ver_cmp(self, other, true, false)
    }
}

impl PartialOrd for NonOpVersion<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use itertools::Itertools;

    use crate::test::TEST_DATA;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn test_new() {
        assert!(Version::new("2").is_ok());
        assert!(Version::new(">=2").is_ok());
    }

    #[test]
    fn test_overflow_version() {
        let u64_max: u128 = u64::MAX as u128;

        for (s1, s2) in [
            // major version
            (format!("{u64_max}"), format!("{}", u64_max + 1)),
            // minor version
            (format!("1.{u64_max}"), format!("1.{}", u64_max + 1)),
            // suffix version
            (format!("1_p{u64_max}"), format!("1_p{}", u64_max + 1)),
            // revision
            (format!("1-r{u64_max}"), format!("1-r{}", u64_max + 1)),
        ] {
            // at bounds limit
            let v1 = Version::from_str(&s1);
            assert!(v1.is_ok());
            // above bounds limit
            let v2 = Version::from_str(&s2);
            assert!(v2.is_err());
        }
    }

    #[test]
    fn test_cmp() {
        let op_map: HashMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        for (expr, (v1, op, v2)) in TEST_DATA.version_toml.compares() {
            let v1 = Version::from_str(v1).unwrap();
            let v2 = Version::from_str(v2).unwrap();
            if op == "!=" {
                assert_ne!(v1, v2, "failed comparing {expr}");
                assert_ne!(v2, v1, "failed comparing {expr}");
            } else {
                let op = op_map[op];
                assert_eq!(v1.cmp(&v2), op, "failed comparing {expr}");
                assert_eq!(v2.cmp(&v1), op.reverse(), "failed comparing {expr}");

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(v1), hash(v2), "failed hash {expr}");
                }
            }
        }
    }

    #[test]
    fn test_intersects() {
        for d in &TEST_DATA.version_toml.intersects {
            // test intersections between all pairs of distinct values
            for vals in d.vals.iter().map(|s| s.as_str()).permutations(2) {
                let v1 = Version::new(vals[0]).unwrap();
                let v2 = Version::new(vals[1]).unwrap();

                // elements intersect themselves
                assert!(v1.intersects(&v1), "{v1} doesn't intersect {v2}");
                assert!(v2.intersects(&v2), "{v2} doesn't intersect {v2}");

                // intersects depending on status
                if d.status {
                    assert!(v1.intersects(&v2), "{v1} doesn't intersect {v2}");
                } else {
                    assert!(!v1.intersects(&v2), "{v1} intersects {v2}");
                }
            }
        }
    }

    #[test]
    fn test_sorting() {
        for d in &TEST_DATA.version_toml.sorting {
            let mut reversed: Vec<_> = d
                .sorted
                .iter()
                .map(|s| Version::from_str(s).unwrap())
                .rev()
                .collect();
            reversed.sort();
            let mut sorted: Vec<_> = reversed.iter().map(|x| x.to_string()).collect();
            if d.equal {
                // equal versions aren't sorted so reversing should restore the original order
                sorted = sorted.into_iter().rev().collect();
            }
            assert_eq!(&sorted, &d.sorted);
        }
    }

    #[test]
    fn test_hashing() {
        for d in &TEST_DATA.version_toml.hashing {
            let set: HashSet<_> = d
                .versions
                .iter()
                .map(|s| Version::from_str(s).unwrap())
                .collect();
            if d.equal {
                assert_eq!(set.len(), 1, "failed hashing versions: {set:?}");
            } else {
                assert_eq!(set.len(), d.versions.len(), "failed hashing versions: {set:?}");
            }
        }
    }
}
