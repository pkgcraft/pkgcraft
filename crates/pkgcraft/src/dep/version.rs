use std::cmp::Ordering;
use std::fmt::{self, Write};
use std::hash::{Hash, Hasher};
use std::str;
use std::str::FromStr;

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};

use crate::macros::cmp_not_equal;
use crate::traits::IntoOwned;
use crate::Error;

use super::{parse, Intersects};

#[derive(Debug, Copy, Clone)]
pub(crate) struct ParsedNumber<'a> {
    pub(crate) raw: &'a str,
    pub(crate) value: u64,
}

impl IntoOwned for ParsedNumber<'_> {
    type Owned = Number;

    fn into_owned(self) -> Self::Owned {
        Number {
            raw: self.raw.to_string(),
            value: self.value,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub(crate) struct Number {
    raw: String,
    value: u64,
}

impl AsRef<str> for Number {
    fn as_ref(&self) -> &str {
        &self.raw
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for Number {}

impl PartialEq<u64> for Number {
    fn eq(&self, other: &u64) -> bool {
        &self.value == other
    }
}

impl PartialEq<str> for Number {
    fn eq(&self, other: &str) -> bool {
        self.raw == other
    }
}

impl PartialEq<&str> for Number {
    fn eq(&self, other: &&str) -> bool {
        self.raw == *other
    }
}

impl PartialEq<Number> for u64 {
    fn eq(&self, other: &Number) -> bool {
        other == self
    }
}

impl PartialEq<Number> for str {
    fn eq(&self, other: &Number) -> bool {
        other == self
    }
}

impl PartialEq<Number> for &str {
    fn eq(&self, other: &Number) -> bool {
        other == *self
    }
}

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

impl FromStr for Number {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let value = s
            .parse()
            .map_err(|e| Error::Overflow(format!("invalid number: {e}: {s}")))?;
        Ok(Self { raw: s.to_string(), value })
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Revision(Number);

impl Revision {
    /// Create a new Revision from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        if s.is_empty() {
            Ok(Self::default())
        } else {
            let value = s
                .parse()
                .map_err(|e| Error::Overflow(format!("invalid revision: {e}: {s}")))?;
            Ok(Self(Number { raw: s.to_string(), value }))
        }
    }
}

impl AsRef<str> for Revision {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl PartialEq<u64> for Revision {
    fn eq(&self, other: &u64) -> bool {
        &self.0.value == other
    }
}

impl PartialEq<str> for Revision {
    fn eq(&self, other: &str) -> bool {
        self.0.raw == other
    }
}

impl PartialEq<&str> for Revision {
    fn eq(&self, other: &&str) -> bool {
        self.0.raw == *other
    }
}

impl PartialEq<Revision> for u64 {
    fn eq(&self, other: &Revision) -> bool {
        other == self
    }
}

impl PartialEq<Revision> for str {
    fn eq(&self, other: &Revision) -> bool {
        other == self
    }
}

impl PartialEq<Revision> for &str {
    fn eq(&self, other: &Revision) -> bool {
        other == *self
    }
}

impl FromStr for Revision {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::new(s)
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(
    Debug, Display, Serialize, Deserialize, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum SuffixKind {
    Alpha,
    Beta,
    Pre,
    Rc,
    P,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct ParsedSuffix<'a> {
    pub(crate) kind: SuffixKind,
    pub(crate) version: Option<ParsedNumber<'a>>,
}

impl IntoOwned for ParsedSuffix<'_> {
    type Owned = Suffix;

    fn into_owned(self) -> Self::Owned {
        Suffix {
            kind: self.kind,
            version: self.version.into_owned(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
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

#[derive(Debug)]
pub(crate) struct ParsedVersion<'a> {
    pub(crate) op: Option<Operator>,
    pub(crate) numbers: Vec<ParsedNumber<'a>>,
    pub(crate) letter: Option<char>,
    pub(crate) suffixes: Vec<ParsedSuffix<'a>>,
    pub(crate) revision: Option<ParsedNumber<'a>>,
}

impl<'a> ParsedVersion<'a> {
    /// Used by the parser to inject the version operator.
    pub(crate) fn with_op(mut self, op: Operator) -> Self {
        self.op = Some(op);
        self
    }
}

impl IntoOwned for ParsedVersion<'_> {
    type Owned = Version;

    fn into_owned(self) -> Self::Owned {
        Version {
            op: self.op,
            numbers: self.numbers.into_iter().map(|x| x.into_owned()).collect(),
            letter: self.letter,
            suffixes: self.suffixes.into_iter().map(|x| x.into_owned()).collect(),
            revision: Revision(self.revision.into_owned().unwrap_or_default()),
        }
    }
}

#[repr(C)]
#[derive(
    AsRefStr,
    Display,
    EnumString,
    EnumIter,
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
            EqualGlob => rhs.starts_with(lhs),
            Approximate => NonRevisionVersion(rhs) == NonRevisionVersion(lhs),
            GreaterOrEqual => NonOpVersion(rhs) >= NonOpVersion(lhs),
            Greater => NonOpVersion(rhs) > NonOpVersion(lhs),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Version {
    pub(super) op: Option<Operator>,
    numbers: Vec<Number>,
    letter: Option<char>,
    suffixes: Vec<Suffix>,
    revision: Revision,
}

impl Version {
    /// Verify a string represents a valid version.
    pub fn valid(s: &str) -> crate::Result<()> {
        if s.starts_with(|c| Operator::iter().any(|op| op.as_ref().starts_with(c))) {
            parse::version_with_op_str(s)?;
        } else {
            parse::version_str(s)?;
        }
        Ok(())
    }

    /// Create a new Version from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        if s.starts_with(|c| Operator::iter().any(|op| op.as_ref().starts_with(c))) {
            parse::version_with_op(s)
        } else {
            parse::version(s)
        }
    }

    /// Return a version's operator, if one exists.
    pub fn op(&self) -> Option<Operator> {
        self.op
    }

    /// Return a version's revision.
    pub fn revision(&self) -> Option<&Revision> {
        if self.revision.0.raw.is_empty() {
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
    fn starts_with(&self, other: &Self) -> bool {
        // flag denoting the lhs has more components than the rhs
        let mut unmatched = false;

        // compare components
        let mut v1_numbers = self.numbers.iter();
        let mut v2_numbers = other.numbers.iter();
        loop {
            match (v1_numbers.next(), v2_numbers.next()) {
                (Some(n1), Some(n2)) => {
                    if !n1.raw.starts_with(&n2.raw) {
                        return false;
                    }
                }
                (None, Some(_)) => return false,
                (Some(_), None) => {
                    unmatched = true;
                    break;
                }
                (None, None) => break,
            }
        }

        // compare letters
        match (&self.letter, &other.letter) {
            (_, Some(_)) if unmatched => return false,
            (Some(c1), Some(c2)) => {
                if c1 != c2 {
                    return false;
                }
            }
            (None, Some(_)) => return false,
            (Some(_), None) => unmatched = true,
            (None, None) => (),
        }

        // compare suffixes
        let mut v1_suffixes = self.suffixes.iter();
        let mut v2_suffixes = other.suffixes.iter();
        loop {
            match (v1_suffixes.next(), v2_suffixes.next()) {
                (_, Some(_)) if unmatched => return false,
                (Some(s1), Some(s2)) => {
                    if s1.kind != s2.kind {
                        return false;
                    }

                    // compare suffix versions
                    match (&s1.version, &s2.version) {
                        (_, Some(_)) if unmatched => return false,
                        (Some(v1), Some(v2)) => {
                            if !v1.raw.starts_with(&v2.raw) {
                                return false;
                            }
                        }
                        (None, Some(_)) => return false,
                        (Some(_), None) => {
                            unmatched = true;
                            break;
                        }
                        (None, None) => (),
                    }
                }
                (None, Some(_)) => return false,
                (Some(_), None) => {
                    unmatched = true;
                    break;
                }
                (None, None) => break,
            }
        }

        // compare revisions
        match (self.revision().map(|r| r.as_ref()), other.revision().map(|r| r.as_ref())) {
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
                None | Some("0") => $ranged.starts_with($other),
                _ => false,
            },

            // remaining '=*' -- intersects if glob matches ranged version
            (_, EqualGlob) => $ranged.starts_with($other),

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
        ver_str(f, self, true, true)
    }
}

fn ver_str(f: &mut fmt::Formatter, v: &Version, rev: bool, op: bool) -> fmt::Result {
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
            None => write!(f, "{s}")?,
            Some(Operator::Less) => write!(f, "<{s}")?,
            Some(Operator::LessOrEqual) => write!(f, "<={s}")?,
            Some(Operator::Equal) => write!(f, "={s}")?,
            Some(Operator::EqualGlob) => write!(f, "={s}*")?,
            Some(Operator::Approximate) => write!(f, "~{s}")?,
            Some(Operator::GreaterOrEqual) => write!(f, ">={s}")?,
            Some(Operator::Greater) => write!(f, ">{s}")?,
        }
    } else {
        write!(f, "{s}")?;
    }

    Ok(())
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Version {}

impl Hash for Version {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.numbers[0].hash(state);
        for n in &self.numbers[1..] {
            if n.raw.starts_with('0') {
                n.raw.trim_end_matches('0').hash(state);
            } else {
                n.value.hash(state);
            }
        }
        self.letter.hash(state);
        self.suffixes.hash(state);
        self.revision.hash(state);
    }
}

fn ver_cmp(v1: &Version, v2: &Version, rev: bool, op: bool) -> Ordering {
    // compare major versions
    cmp_not_equal!(&v1.numbers[0], &v2.numbers[0]);

    // compare remaining version components
    let mut v1_numbers = v1.numbers[1..].iter();
    let mut v2_numbers = v2.numbers[1..].iter();
    loop {
        match (v1_numbers.next(), v2_numbers.next()) {
            // compare as strings if a component starts with "0"
            (Some(n1), Some(n2)) if n1.raw.starts_with('0') || n2.raw.starts_with('0') => {
                cmp_not_equal!(n1.raw.trim_end_matches('0'), n2.raw.trim_end_matches('0'))
            }
            // compare as integers
            (Some(n1), Some(n2)) => cmp_not_equal!(n1, n2),
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
            (Some(Suffix { kind: SuffixKind::P, .. }), None) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (None, Some(Suffix { kind: SuffixKind::P, .. })) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => break,
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
struct NonRevisionVersion<'a>(&'a Version);

impl PartialEq for NonRevisionVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        ver_cmp(self.0, other.0, false, false) == Ordering::Equal
    }
}

impl Eq for NonRevisionVersion<'_> {}

impl fmt::Display for NonRevisionVersion<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        ver_str(f, self.0, false, false)
    }
}

/// Version wrapper that ignores operators during comparisons.
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

impl fmt::Display for NonOpVersion<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        ver_str(f, self.0, true, false)
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
    fn ver_new_and_valid() {
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
    fn rev_new_and_parse() {
        // invalid
        for s in ["a", "a1", "1.1", ".1"] {
            assert!(s.parse::<Revision>().is_err());
            assert!(Revision::new(s).is_err());
        }

        // empty
        let rev = Revision::new("").unwrap();
        assert_eq!(rev, 0);
        assert_eq!(0, rev);
        assert_eq!(rev, "");
        assert_eq!("", rev);
        assert_eq!(rev, Revision::default());
        assert_eq!(rev.to_string(), "");

        // simple integer
        let rev1 = Revision::new("1").unwrap();
        assert_eq!(rev1, 1);
        assert_eq!(1, rev1);
        assert_eq!(rev1, "1");
        assert_eq!("1", rev1);
        assert_eq!(rev1.to_string(), "1");

        // zero prefixes are technically allowed
        let rev2 = Revision::new("01").unwrap();
        assert_eq!(rev2, 1);
        assert_eq!(1, rev2);
        assert_eq!(rev2, "01");
        assert_eq!("01", rev2);
        assert_eq!(rev2.to_string(), "01");
        assert_eq!(rev1, rev2);
    }

    #[test]
    fn ver_compare() {
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
    fn ver_intersects() {
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
    fn ver_sorting() {
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
    fn ver_hashing() {
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
