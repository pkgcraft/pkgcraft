use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
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

impl Revision {
    /// Create a new Revision from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        let rev = if s.is_empty() {
            Self::default()
        } else {
            Self {
                value: Some(s.to_string()),
                int: s
                    .parse()
                    .map_err(|e| Error::Overflow(format!("invalid revision: {e}: {s}")))?,
            }
        };

        Ok(rev)
    }
}

impl FromStr for Revision {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::new(s)
    }
}

impl AsRef<str> for Revision {
    fn as_ref(&self) -> &str {
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
        Some(self.cmp(other))
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[derive(Debug)]
pub(crate) struct ParsedVersion<'a> {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) base_end: usize,
    pub(crate) op: Option<Operator>,
    pub(crate) numbers: Vec<(&'a str, u64)>,
    pub(crate) letter: Option<char>,
    pub(crate) suffixes: Vec<Suffix>,
    pub(crate) revision: Option<(&'a str, u64)>,
}

impl<'a> ParsedVersion<'a> {
    // Used by the parser to inject the version operator value.
    pub(crate) fn with_op(
        mut self,
        op: &'a str,
        glob: Option<&'a str>,
    ) -> Result<Self, &'static str> {
        use Operator::*;
        let op = match (op, glob, self.revision) {
            ("<", None, _) => Ok(Less),
            ("<=", None, _) => Ok(LessOrEqual),
            ("=", None, _) => Ok(Equal),
            ("=", Some(_), _) => Ok(EqualGlob),
            ("~", None, None) => Ok(Approximate),
            ("~", None, Some(_)) => Err("~ version operator can't be used with a revision"),
            (">=", None, _) => Ok(GreaterOrEqual),
            (">", None, _) => Ok(Greater),
            _ => Err("invalid version operator"),
        }?;

        self.op = Some(op);
        Ok(self)
    }

    pub(crate) fn into_owned(self, input: &str) -> Version {
        let numbers = self
            .numbers
            .into_iter()
            .map(|(s, n)| (s.to_string(), n))
            .collect();

        let revision = self
            .revision
            .map(|(s, n)| Revision {
                value: Some(s.to_string()),
                int: n,
            })
            .unwrap_or_default();

        Version {
            full: input[self.start..self.end].to_string(),
            base_end: self.base_end,
            op: self.op,
            numbers,
            letter: self.letter,
            suffixes: self.suffixes,
            revision,
        }
    }
}

#[repr(C)]
#[derive(
    AsRefStr,
    Display,
    EnumString,
    Debug,
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
    #[strum(serialize = "<")]
    Less = 1,
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
    fn intersects(&self, lhs: &Version, rhs: &Version) -> bool {
        use Operator::*;
        match self {
            Less => NonOpVersion(rhs) < NonOpVersion(lhs),
            LessOrEqual => NonOpVersion(rhs) <= NonOpVersion(lhs),
            Equal => NonOpVersion(rhs) == NonOpVersion(lhs),
            EqualGlob => rhs.as_str().starts_with(lhs.as_str()),
            Approximate => NonRevisionVersion(rhs) == NonRevisionVersion(lhs),
            GreaterOrEqual => NonOpVersion(rhs) >= NonOpVersion(lhs),
            Greater => NonOpVersion(rhs) > NonOpVersion(lhs),
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
        parse::version_str(s).or_else(|_| parse::version_with_op_str(s))?;
        Ok(())
    }

    /// Create a new Version from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        parse::version(s).or_else(|_| parse::version_with_op(s))
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

// unbounded operators
macro_rules! unbounded {
    () => {
        Operator::Less | Operator::LessOrEqual | Operator::Greater | Operator::GreaterOrEqual
    };
}

// handle remaining ranged intersections
macro_rules! ranged {
    ($ranged:expr, $ranged_op:expr, $other:expr, $other_op:expr) => {
        match ($ranged_op, $other_op) {
            // '~' or '=*' -- intersects if range matches
            (op, Approximate | EqualGlob) if op.intersects($ranged, $other) => true,

            // remaining '~' -- intersects if ranged is '>' or '>=' on other's version with
            // a nonzero revision, e.g. >1-r1 intersects with ~1
            (Greater | GreaterOrEqual, Approximate) => $other_op.intersects($other, $ranged),
            (_, Approximate) => false,

            // '=*' and '<' or '<=' -- intersects if the other revision is 0 or doesn't
            // exist and glob matches ranged version
            (Less | LessOrEqual, EqualGlob) => match $other.revision().map(|r| r.as_ref()) {
                None | Some("0") => $ranged.as_str().starts_with($other.as_str()),
                _ => false,
            },

            // remaining '=*' -- intersects if glob matches ranged version
            (_, EqualGlob) => $ranged.as_str().starts_with($other.as_str()),

            // remaining variants should never occur
            (_, op) => unreachable!("{op:?} operator should be previously handled"),
        }
    };
}

/// Determine if two versions intersect.
impl Intersects<Version> for Version {
    fn intersects(&self, other: &Version) -> bool {
        use Operator::*;
        match (self.op, other.op) {
            // intersects if both are unbounded in the same direction
            (Some(Less | LessOrEqual), Some(Less | LessOrEqual)) => true,
            (Some(Greater | GreaterOrEqual), Some(Greater | GreaterOrEqual)) => true,

            // unbounded in opposite directions -- intersects if both match
            (Some(lhs @ unbounded!()), Some(rhs @ unbounded!())) => {
                lhs.intersects(self, other) && rhs.intersects(other, self)
            }

            // both non-op or '~' -- intersects if equal
            (None, None) | (Some(Approximate), Some(Approximate)) => self == other,

            // either non-op or '=' -- intersects if the other matches it
            (Some(op), None | Some(Equal)) => op.intersects(self, other),
            (None | Some(Equal), Some(op)) => op.intersects(other, self),

            // both '=*' -- intersects if either glob matches
            (Some(op @ EqualGlob), Some(EqualGlob)) => {
                op.intersects(self, other) || op.intersects(other, self)
            }

            // '=*' and '~' -- intersects if glob matches unrevisioned version
            (Some(EqualGlob), Some(Approximate)) => other.as_str().starts_with(self.base()),
            (Some(Approximate), Some(EqualGlob)) => self.as_str().starts_with(other.base()),

            // remaining cases must have one op that is unbounded
            (Some(lhs @ unbounded!()), Some(rhs)) => ranged!(self, lhs, other, rhs),
            (Some(lhs), Some(rhs @ unbounded!())) => ranged!(other, rhs, self, lhs),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Operator::*;

        let s = self.as_str();
        match self.op {
            None => write!(f, "{}", s),
            Some(Less) => write!(f, "<{}", s),
            Some(LessOrEqual) => write!(f, "<={}", s),
            Some(Equal) => write!(f, "={}", s),
            Some(EqualGlob) => write!(f, "={}*", s),
            Some(Approximate) => write!(f, "~{}", s),
            Some(GreaterOrEqual) => write!(f, ">={}", s),
            Some(Greater) => write!(f, ">{}", s),
        }
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

fn ver_cmp(v1: &Version, v2: &Version, cmp_revs: bool, cmp_ops: bool) -> Ordering {
    if v1.base() != v2.base() {
        // compare major versions
        cmp_not_equal!(&v1.numbers[0].1, &v2.numbers[0].1);

        // compare remaining version components
        let mut v1_numbers = v1.numbers[1..].iter();
        let mut v2_numbers = v2.numbers[1..].iter();
        loop {
            match (v1_numbers.next(), v2_numbers.next()) {
                // lexical equality implies numerical equality
                (Some((s1, _)), Some((s2, _))) if s1 == s2 => continue,
                // compare as strings if a component starts with "0"
                (Some((s1, _)), Some((s2, _))) if s1.starts_with('0') || s2.starts_with('0') => {
                    cmp_not_equal!(s1.trim_end_matches('0'), s2.trim_end_matches('0'))
                }
                // compare as integers
                (Some((_, n1)), Some((_, n2))) => cmp_not_equal!(n1, n2),
                (Some(_), None) => return Ordering::Greater,
                (None, Some(_)) => return Ordering::Less,
                (None, None) => break,
            }
        }

        // compare letter suffixes
        cmp_not_equal!(&v1.letter, &v2.letter);

        // compare suffixes
        let mut v1_suffixes = v1.suffixes.iter();
        let mut v2_suffixes = v2.suffixes.iter();
        loop {
            match (v1_suffixes.next(), v2_suffixes.next()) {
                (Some(s1), Some(s2)) => cmp_not_equal!(s1, s2),
                (Some(Suffix::P(_)), None) => return Ordering::Greater,
                (Some(_), None) => return Ordering::Less,
                (None, Some(Suffix::P(_))) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (None, None) => break,
            }
        }
    }

    // compare revisions
    if cmp_revs {
        cmp_not_equal!(&v1.revision, &v2.revision);
    }

    // compare operators
    if cmp_ops {
        cmp_not_equal!(&v1.op, &v2.op);
    }

    Ordering::Equal
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
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
#[derive(Debug)]
struct NonRevisionVersion<'a>(&'a Version);

impl PartialEq for NonRevisionVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        ver_cmp(self.0, other.0, false, false) == Ordering::Equal
    }
}

impl Eq for NonRevisionVersion<'_> {}

/// Version wrapper that ignores operators during comparisons.
#[derive(Debug)]
struct NonOpVersion<'a>(&'a Version);

impl PartialEq for NonOpVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for NonOpVersion<'_> {}

impl Ord for NonOpVersion<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        ver_cmp(self.0, other.0, true, false)
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
    fn test_new_and_valid() {
        // invalid
        for s in &TEST_DATA.version_toml.invalid {
            let result = Version::valid(s);
            assert!(result.is_err(), "{s:?} is valid");
            let result = Version::new(s);
            assert!(result.is_err(), "{s:?} didn't fail");
        }

        // valid
        for s in &TEST_DATA.version_toml.valid {
            let result = Version::valid(s);
            assert!(result.is_ok(), "{s:?} is invalid");
            let result = Version::new(s);
            assert!(result.is_ok(), "{s:?} failed");
        }
    }

    #[test]
    fn test_cmp() {
        let op_map: HashMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        for (expr, (s1, op, s2)) in TEST_DATA.version_toml.compares() {
            let v1: Version = s1.parse().unwrap();
            let v2: Version = s2.parse().unwrap();
            if op == "!=" {
                assert_ne!(v1, v2, "failed comparing: {expr}");
                assert_ne!(v2, v1, "failed comparing: {expr}");
            } else {
                let op = op_map[op];
                assert_eq!(v1.cmp(&v2), op, "failed comparing: {expr}");
                assert_eq!(v2.cmp(&v1), op.reverse(), "failed comparing inverted: {expr}");

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(v1), hash(v2), "failed hash: {expr}");
                }
            }
        }
    }

    #[test]
    fn test_intersects() {
        for d in &TEST_DATA.version_toml.intersects {
            // test intersections between all pairs of distinct values
            let permutations = d
                .vals
                .iter()
                .map(|s| s.as_str())
                .permutations(2)
                .map(|val| val.into_iter().collect_tuple().unwrap());
            for (s1, s2) in permutations {
                let v1: Version = s1.parse().unwrap();
                let v2: Version = s2.parse().unwrap();

                // self intersection
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
            let mut reversed: Vec<Version> =
                d.sorted.iter().map(|s| s.parse().unwrap()).rev().collect();
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
            let set: HashSet<Version> = d.versions.iter().map(|s| s.parse().unwrap()).collect();
            if d.equal {
                assert_eq!(set.len(), 1, "failed hashing versions: {set:?}");
            } else {
                assert_eq!(set.len(), d.versions.len(), "failed hashing versions: {set:?}");
            }
        }
    }
}
