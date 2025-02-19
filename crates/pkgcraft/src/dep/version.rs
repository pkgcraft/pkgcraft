use std::cmp::Ordering;
use std::fmt::{self, Write};
use std::hash::{Hash, Hasher};
use std::str;
use std::str::FromStr;

use itertools::EitherOrBoth::{Both, Left, Right};
use itertools::Itertools;
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};

use crate::macros::cmp_not_equal;
use crate::traits::Intersects;
use crate::Error;

use super::parse;

/// Modify or create a new type by adding a version operator.
pub trait WithOp {
    type WithOp;
    fn with_op(self, op: Operator) -> Result<Self::WithOp, &'static str>;
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub(crate) struct Number {
    pub(crate) raw: String,
    pub(crate) value: u64,
}

impl Number {
    /// Determine if a number is represented by an empty string.
    pub(crate) fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// Return the raw string value for a number.
    pub(crate) fn as_str(&self) -> &str {
        &self.raw
    }

    /// Determine if a Number starts with another Number using string representation.
    fn starts_with(&self, other: &Number) -> bool {
        self.raw.starts_with(&other.raw)
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for Number {}

impl Hash for Number {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Revision(pub(crate) Number);

impl Revision {
    /// Create a new Revision from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
        if s.is_empty() {
            Ok(Self::default())
        } else {
            Ok(parse::revision(s)?)
        }
    }

    /// Determine if a revision is represented by an empty string.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return the raw string value for a revision.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Determine if a Revision starts with another Revision using string representation.
    fn starts_with(&self, other: &Revision) -> bool {
        self.0.starts_with(&other.0)
    }
}

impl FromStr for Revision {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Display, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[strum(serialize_all = "snake_case")]
#[cfg_attr(test, derive(serde::Serialize))]
pub(crate) enum SuffixKind {
    Alpha,
    Beta,
    Pre,
    Rc,
    P,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub(crate) struct Suffix {
    pub(crate) kind: SuffixKind,
    pub(crate) version: Option<Number>,
}

impl fmt::Display for Suffix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(ver) = self.version.as_ref() {
            write!(f, "{ver}")?;
        }
        Ok(())
    }
}

#[repr(C)]
#[derive(
    AsRefStr,
    Display,
    EnumString,
    EnumIter,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
)]
#[cfg_attr(test, derive(serde::Serialize))]
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
    /// Determine if two versions intersect for an operator.
    fn intersects(&self, lhs: &Version, rhs: &Version) -> bool {
        match self {
            Self::Less => NonOpVersion(rhs) < NonOpVersion(lhs),
            Self::LessOrEqual => NonOpVersion(rhs) <= NonOpVersion(lhs),
            Self::Equal => NonOpVersion(rhs) == NonOpVersion(lhs),
            Self::EqualGlob => rhs.starts_with(lhs),
            Self::Approximate => NonRevisionVersion(rhs) == NonRevisionVersion(lhs),
            Self::GreaterOrEqual => NonOpVersion(rhs) >= NonOpVersion(lhs),
            Self::Greater => NonOpVersion(rhs) > NonOpVersion(lhs),
        }
    }
}

#[derive(Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Version {
    pub(crate) op: Option<Operator>,
    pub(crate) numbers: Vec<Number>,
    pub(crate) letter: Option<char>,
    pub(crate) suffixes: Vec<Suffix>,
    pub(crate) revision: Revision,
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Version {{ {self} }}")
    }
}

impl WithOp for Version {
    type WithOp = Version;

    fn with_op(mut self, op: Operator) -> Result<Self::WithOp, &'static str> {
        if op == Operator::Approximate && !self.revision.is_empty() {
            Err("~ version operator can't be used with a revision")
        } else {
            self.op = Some(op);
            Ok(self)
        }
    }
}

impl Version {
    /// Create a [`Version`] from a given string with or without an [`Operator`].
    pub fn try_new<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        let s = s.as_ref();
        if s.starts_with(|c| Operator::iter().any(|op| op.as_ref().starts_with(c))) {
            parse::version_with_op(s)
        } else {
            parse::version(s)
        }
    }

    /// Create a [`Version`] with an [`Operator`].
    pub fn try_new_with_op<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        parse::version_with_op(s.as_ref())
    }

    /// Create a [`Version`] without an [`Operator`].
    pub fn try_new_without_op<S: AsRef<str>>(s: S) -> crate::Result<Self> {
        parse::version(s.as_ref())
    }
}

impl Version {
    /// Return a version's operator, if one exists.
    pub fn op(&self) -> Option<Operator> {
        self.op
    }

    /// Return a version's revision.
    pub fn revision(&self) -> Option<&Revision> {
        if self.revision.is_empty() {
            None
        } else {
            Some(&self.revision)
        }
    }

    /// Return a version's string value without operator.
    pub fn without_op(&self) -> String {
        NonOpVersion(self).to_string()
    }

    /// Return a version's string value without operator or revision.
    pub fn base(&self) -> String {
        NonRevisionVersion(self).to_string()
    }

    /// Determine if a Version starts with another Version, disregarding the operator.
    fn starts_with(&self, other: &Version) -> bool {
        // flag denoting the lhs has more components than the rhs
        let mut unmatched = false;

        // compare components
        for numbers in self.numbers.iter().zip_longest(&other.numbers) {
            match numbers {
                Both(n1, n2) => {
                    if !n1.starts_with(n2) {
                        return false;
                    }
                }
                Left(_) => {
                    unmatched = true;
                    break;
                }
                Right(_) => return false,
            }
        }

        // compare letters
        match (&self.letter, &other.letter) {
            (Some(c1), Some(c2)) => {
                if unmatched || c1 != c2 {
                    return false;
                }
            }
            (None, Some(_)) => return false,
            (Some(_), None) => unmatched = true,
            (None, None) => (),
        }

        // compare suffixes
        for suffixes in self.suffixes.iter().zip_longest(&other.suffixes) {
            match suffixes {
                Both(s1, s2) => {
                    if unmatched || s1.kind != s2.kind {
                        return false;
                    }

                    // compare suffix versions
                    match (&s1.version, &s2.version) {
                        (Some(v1), Some(v2)) => {
                            if !v1.starts_with(v2) {
                                return false;
                            }
                        }
                        (None, Some(_)) => return false,
                        (Some(_), None) => unmatched = true,
                        (None, None) => (),
                    }
                }
                Left(_) => {
                    unmatched = true;
                    break;
                }
                Right(_) => return false,
            }
        }

        // compare revisions
        match (self.revision(), other.revision()) {
            (Some(r1), Some(r2)) if unmatched || !r1.starts_with(r2) => false,
            (None, Some(_)) => false,
            _ => true,
        }
    }
}

// unbounded operators
macro_rules! unbounded {
    () => {
        Operator::Less | Operator::LessOrEqual | Operator::Greater | Operator::GreaterOrEqual
    };
}

/// Determine if two ranged versions intersect.
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
            (Less | LessOrEqual, EqualGlob) => match $other.revision().map(|r| r.as_str()) {
                None | Some("0") => $ranged.starts_with($other),
                _ => false,
            },

            // remaining '=*' -- intersects if glob matches ranged version
            (_, EqualGlob) => $ranged.starts_with($other),

            // remaining variants should never occur
            (_, op) => unreachable!("operator should be previously handled: {op:?}"),
        }
    };
}

impl Intersects<Self> for Version {
    fn intersects(&self, other: &Self) -> bool {
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
            (Some(EqualGlob), Some(Approximate)) => other.starts_with(self),
            (Some(Approximate), Some(EqualGlob)) => self.starts_with(other),

            // remaining cases must have one op that is unbounded
            (Some(lhs @ unbounded!()), Some(rhs)) => ranged!(self, lhs, other, rhs),
            (Some(lhs), Some(rhs @ unbounded!())) => ranged!(other, rhs, self, lhs),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt(f, self, true, true)
    }
}

/// Format a [`Version`] into a [`String`], optionally ignoring the revision and/or operator.
fn fmt(f: &mut fmt::Formatter, v: &Version, rev: bool, op: bool) -> fmt::Result {
    let mut s = String::new();

    write!(s, "{}", v.numbers.iter().join("."))?;

    if let Some(c) = &v.letter {
        write!(s, "{c}")?;
    }

    for suffix in &v.suffixes {
        write!(s, "_{suffix}")?;
    }

    if rev {
        if let Some(rev) = v.revision() {
            write!(s, "-r{rev}")?;
        }
    }

    if op {
        match &v.op {
            None => write!(f, "{s}"),
            Some(Operator::Less) => write!(f, "<{s}"),
            Some(Operator::LessOrEqual) => write!(f, "<={s}"),
            Some(Operator::Equal) => write!(f, "={s}"),
            Some(Operator::EqualGlob) => write!(f, "={s}*"),
            Some(Operator::Approximate) => write!(f, "~{s}"),
            Some(Operator::GreaterOrEqual) => write!(f, ">={s}"),
            Some(Operator::Greater) => write!(f, ">{s}"),
        }
    } else {
        write!(f, "{s}")
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        cmp(self, other, true, true) == Ordering::Equal
    }
}

impl Eq for Version {}

impl Hash for Version {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.numbers[0].hash(state);
        for n in &self.numbers[1..] {
            if n.as_str().starts_with('0') {
                n.as_str().trim_end_matches('0').hash(state);
            } else {
                n.value.hash(state);
            }
        }
        self.letter.hash(state);
        self.suffixes.hash(state);
        self.revision.hash(state);
    }
}

/// Compare two versions, optionally ignoring the revision and/or operator.
fn cmp(v1: &Version, v2: &Version, rev: bool, op: bool) -> Ordering {
    // compare major versions
    cmp_not_equal!(&v1.numbers[0], &v2.numbers[0]);

    // compare remaining version components
    for numbers in v1.numbers[1..].iter().zip_longest(&v2.numbers[1..]) {
        match numbers {
            Both(n1, n2) => {
                // compare as strings if a component starts with "0", otherwise as integers
                let (s1, s2) = (n1.as_str(), n2.as_str());
                if s1.starts_with('0') || s2.starts_with('0') {
                    cmp_not_equal!(s1.trim_end_matches('0'), s2.trim_end_matches('0'));
                } else {
                    cmp_not_equal!(n1, n2);
                }
            }
            Left(_) => return Ordering::Greater,
            Right(_) => return Ordering::Less,
        }
    }

    // compare letter suffixes
    cmp_not_equal!(&v1.letter, &v2.letter);

    // compare suffixes
    for suffixes in v1.suffixes.iter().zip_longest(&v2.suffixes) {
        match suffixes {
            Both(s1, s2) => cmp_not_equal!(s1, s2),
            Left(s) if s.kind == SuffixKind::P => return Ordering::Greater,
            Left(_) => return Ordering::Less,
            Right(s) if s.kind == SuffixKind::P => return Ordering::Less,
            Right(_) => return Ordering::Greater,
        }
    }

    // compare revisions
    if rev {
        cmp_not_equal!(&v1.revision, &v2.revision);
    }

    // compare operators
    if op {
        cmp_not_equal!(&v1.op, &v2.op);
    }

    Ordering::Equal
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp(self, other, true, true)
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
        Self::try_new(s)
    }
}

/// Version wrapper that ignores revisions and operators during comparisons.
struct NonRevisionVersion<'a>(&'a Version);

impl PartialEq for NonRevisionVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        cmp(self.0, other.0, false, false) == Ordering::Equal
    }
}

impl Eq for NonRevisionVersion<'_> {}

impl fmt::Display for NonRevisionVersion<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt(f, self.0, false, false)
    }
}

/// Version wrapper that ignores operators during comparisons.
struct NonOpVersion<'a>(&'a Version);

impl PartialEq for NonOpVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        cmp(self.0, other.0, true, false) == Ordering::Equal
    }
}

impl Eq for NonOpVersion<'_> {}

impl Ord for NonOpVersion<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp(self.0, other.0, true, false)
    }
}

impl PartialOrd for NonOpVersion<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for NonOpVersion<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt(f, self.0, true, false)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use itertools::Itertools;

    use crate::test::test_data;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn new_and_parse() {
        let data = test_data();

        // invalid
        for s in &data.version_toml.invalid {
            let result = Version::try_new(s);
            assert!(result.is_err(), "{s:?} didn't fail");
        }

        // valid
        for s in &data.version_toml.valid {
            let result = Version::try_new(s);
            assert!(result.is_ok(), "{s:?} failed");
            let ver = result.unwrap();
            assert_eq!(ver.to_string(), s.as_str());
            assert!(format!("{ver:?}").contains(s));
        }

        // forced with and without operators
        assert!(Version::try_new_with_op(">1").is_ok());
        assert!(Version::try_new_with_op("1").is_err());
        assert!(Version::try_new_without_op(">1").is_err());
        assert!(Version::try_new_without_op("1").is_ok());
    }

    #[test]
    fn op() {
        let ver = Version::try_new("1").unwrap();
        assert!(ver.op().is_none());

        for op in Operator::iter() {
            let ver = Version::try_new("1").unwrap().with_op(op).unwrap();
            assert_eq!(ver.op(), Some(op));
        }
    }

    #[test]
    fn rev_new_and_parse() {
        // invalid
        for s in ["a", "a1", "1.1", ".1"] {
            assert!(s.parse::<Revision>().is_err());
            assert!(Revision::try_new(s).is_err());
        }

        // valid
        for s in ["", "1", "01"] {
            let rev = Revision::try_new(s).unwrap();
            assert_eq!(rev.to_string(), s);
        }
    }

    #[test]
    fn compare() {
        let op_map: HashMap<_, _> =
            [("<", Ordering::Less), ("==", Ordering::Equal), (">", Ordering::Greater)]
                .into_iter()
                .collect();

        let data = test_data();
        for (expr, (s1, op, s2)) in data.version_toml.compares() {
            let v1 = Version::try_new(s1).unwrap();
            let v2 = Version::try_new(s2).unwrap();
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
                    assert_eq!(hash(&v1), hash(&v2), "failed hash: {expr}");
                }
            }
        }
    }

    #[test]
    fn intersects() {
        let data = test_data();
        for d in &data.version_toml.intersects {
            // test intersections between all pairs of distinct values
            let permutations = d
                .vals
                .iter()
                .map(|s| s.as_str())
                .permutations(2)
                .map(|val| val.into_iter().collect_tuple().unwrap());
            for (s1, s2) in permutations {
                let v1 = Version::try_new(s1).unwrap();
                let v2 = Version::try_new(s2).unwrap();

                // self intersection
                assert!(v1.intersects(&v1), "{s1} doesn't intersect {s2}");
                assert!(v2.intersects(&v2), "{s2} doesn't intersect {s2}");

                // intersects depending on status
                if d.status {
                    assert!(v1.intersects(&v2), "{s1} doesn't intersect {s2}");
                } else {
                    assert!(!v1.intersects(&v2), "{s1} intersects {s2}");
                }
            }
        }
    }

    #[test]
    fn sorting() {
        let data = test_data();
        for d in &data.version_toml.sorting {
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
    fn hashing() {
        let data = test_data();
        for d in &data.version_toml.hashing {
            let set: HashSet<Version> =
                d.versions.iter().map(|s| s.parse().unwrap()).collect();
            if d.equal {
                assert_eq!(set.len(), 1, "failed hashing versions: {set:?}");
            } else {
                assert_eq!(set.len(), d.versions.len(), "failed hashing versions: {set:?}");
            }
        }
    }
}
