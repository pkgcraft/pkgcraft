use std::cmp::Ordering;
use std::fmt::{self, Write};
use std::hash::{Hash, Hasher};
use std::str;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};

use crate::macros::cmp_not_equal;
use crate::traits::{Intersects, IntoOwned};
use crate::Error;

use super::{parse, Stringable};

/// Modify or create a new type by adding a version operator.
pub(crate) trait WithOp {
    type WithOp;
    fn with_op(self, op: Operator) -> Result<Self::WithOp, &'static str>;
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub(crate) struct Number<S: Stringable> {
    pub(crate) raw: S,
    pub(crate) value: u64,
}

impl IntoOwned for Number<&str> {
    type Owned = Number<String>;

    fn into_owned(self) -> Self::Owned {
        Number {
            raw: self.raw.to_string(),
            value: self.value,
        }
    }
}

impl<S: Stringable> Number<S> {
    /// Determine if a number is represented by an empty string.
    pub(crate) fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }
}

impl<S: Stringable> AsRef<str> for Number<S> {
    fn as_ref(&self) -> &str {
        self.raw.as_ref()
    }
}

impl<S: Stringable> PartialEq for Number<S> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<S: Stringable> Eq for Number<S> {}

impl<S: Stringable> PartialEq<u64> for Number<S> {
    fn eq(&self, other: &u64) -> bool {
        &self.value == other
    }
}

impl<S: Stringable> PartialEq<str> for Number<S> {
    fn eq(&self, other: &str) -> bool {
        self.raw.as_ref() == other
    }
}

impl<S: Stringable> PartialEq<&str> for Number<S> {
    fn eq(&self, other: &&str) -> bool {
        self.raw.as_ref() == *other
    }
}

impl<S: Stringable> PartialEq<Number<S>> for u64 {
    fn eq(&self, other: &Number<S>) -> bool {
        other == self
    }
}

impl<S: Stringable> PartialEq<Number<S>> for str {
    fn eq(&self, other: &Number<S>) -> bool {
        other == self
    }
}

impl<S: Stringable> PartialEq<Number<S>> for &str {
    fn eq(&self, other: &Number<S>) -> bool {
        other == *self
    }
}

impl<S: Stringable> Hash for Number<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<S: Stringable> Ord for Number<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<S: Stringable> PartialOrd for Number<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: Stringable> fmt::Display for Number<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Revision<S: Stringable>(pub(crate) Number<S>);

impl IntoOwned for Revision<&str> {
    type Owned = Revision<String>;

    fn into_owned(self) -> Self::Owned {
        Revision(self.0.into_owned())
    }
}

impl Revision<String> {
    /// Create a new Revision from a given string.
    pub fn new(s: &str) -> crate::Result<Self> {
        if s.is_empty() {
            Ok(Self::default())
        } else {
            Ok(Self(parse::number(s)?.into_owned()))
        }
    }
}

impl<S: Stringable> Revision<S> {
    /// Determine if a revision is represented by an empty string.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<S: Stringable> AsRef<str> for Revision<S> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<S: Stringable> PartialEq<u64> for Revision<S> {
    fn eq(&self, other: &u64) -> bool {
        &self.0.value == other
    }
}

impl<S: Stringable> PartialEq<str> for Revision<S> {
    fn eq(&self, other: &str) -> bool {
        self.as_ref() == other
    }
}

impl<S: Stringable> PartialEq<&str> for Revision<S> {
    fn eq(&self, other: &&str) -> bool {
        self.as_ref() == *other
    }
}

impl<S: Stringable> PartialEq<Revision<S>> for u64 {
    fn eq(&self, other: &Revision<S>) -> bool {
        other == self
    }
}

impl<S: Stringable> PartialEq<Revision<S>> for str {
    fn eq(&self, other: &Revision<S>) -> bool {
        other == self
    }
}

impl<S: Stringable> PartialEq<Revision<S>> for &str {
    fn eq(&self, other: &Revision<S>) -> bool {
        other == *self
    }
}

impl FromStr for Revision<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::new(s)
    }
}

impl<S: Stringable> fmt::Display for Revision<S> {
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub(crate) struct Suffix<S: Stringable> {
    pub(crate) kind: SuffixKind,
    pub(crate) version: Option<Number<S>>,
}

impl IntoOwned for Suffix<&str> {
    type Owned = Suffix<String>;

    fn into_owned(self) -> Self::Owned {
        Suffix {
            kind: self.kind,
            version: self.version.into_owned(),
        }
    }
}

impl<S: Stringable> fmt::Display for Suffix<S> {
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
    fn intersects<S: Stringable>(&self, lhs: &Version<S>, rhs: &Version<S>) -> bool {
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
pub struct Version<S: Stringable> {
    pub(crate) op: Option<Operator>,
    pub(crate) numbers: Vec<Number<S>>,
    pub(crate) letter: Option<char>,
    pub(crate) suffixes: Vec<Suffix<S>>,
    pub(crate) revision: Revision<S>,
}

impl<'a> WithOp for Version<&'a str> {
    type WithOp = Version<&'a str>;

    fn with_op(mut self, op: Operator) -> Result<Self::WithOp, &'static str> {
        if op == Operator::Approximate && !self.revision.is_empty() {
            Err("~ version operator can't be used with a revision")
        } else {
            self.op = Some(op);
            Ok(self)
        }
    }
}

impl IntoOwned for Version<&str> {
    type Owned = Version<String>;

    fn into_owned(self) -> Self::Owned {
        Version {
            op: self.op,
            numbers: self.numbers.into_iter().map(|x| x.into_owned()).collect(),
            letter: self.letter,
            suffixes: self.suffixes.into_iter().map(|x| x.into_owned()).collect(),
            revision: self.revision.into_owned(),
        }
    }
}

impl<'a> Version<&'a str> {
    /// Create a borrowed [`Version`] from a given string with or without an [`Operator`].
    pub fn parse(s: &'a str) -> crate::Result<Self> {
        if s.starts_with(|c| Operator::iter().any(|op| op.as_ref().starts_with(c))) {
            parse::version_with_op(s)
        } else {
            parse::version(s)
        }
    }
}

impl Version<String> {
    /// Create a new [`Version`] from a given string with or without an [`Operator`].
    pub fn new(s: &str) -> crate::Result<Self> {
        Version::parse(s).into_owned()
    }

    /// Create a new [`Version`] with an [`Operator`].
    pub fn new_with_op(s: &str) -> crate::Result<Self> {
        parse::version_with_op(s).into_owned()
    }

    /// Create a new [`Version`] without an [`Operator`].
    pub fn new_without_op(s: &str) -> crate::Result<Self> {
        parse::version(s).into_owned()
    }

    /// Verify a string represents a valid version.
    pub fn valid(s: &str) -> crate::Result<()> {
        if s.starts_with(|c| Operator::iter().any(|op| op.as_ref().starts_with(c))) {
            parse::version_with_op(s)?;
        } else {
            parse::version(s)?;
        }
        Ok(())
    }
}

impl<S: Stringable> Version<S> {
    /// Modify the version's operator.
    pub(crate) fn with_op(&mut self, op: Operator) {
        self.op = Some(op);
    }

    /// Return a version's operator, if one exists.
    pub fn op(&self) -> Option<Operator> {
        self.op
    }

    /// Return a version's revision.
    pub fn revision(&self) -> Option<&Revision<S>> {
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
    fn starts_with(&self, other: &Self) -> bool {
        // flag denoting the lhs has more components than the rhs
        let mut unmatched = false;

        // compare components
        let mut v1_numbers = self.numbers.iter();
        let mut v2_numbers = other.numbers.iter();
        loop {
            match (v1_numbers.next(), v2_numbers.next()) {
                (Some(n1), Some(n2)) => {
                    if !n1.as_ref().starts_with(n2.as_ref()) {
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
                        (Some(v1), Some(v2)) => {
                            if !v1.as_ref().starts_with(v2.as_ref()) {
                                return false;
                            }
                        }
                        (None, Some(_)) => return false,
                        (Some(_), None) => unmatched = true,
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
impl<S: Stringable> Intersects<Version<S>> for Version<S> {
    fn intersects(&self, other: &Version<S>) -> bool {
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

impl<S: Stringable> fmt::Display for Version<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        ver_str(f, self, true, true)
    }
}

fn ver_str<S: Stringable>(
    f: &mut fmt::Formatter,
    v: &Version<S>,
    rev: bool,
    op: bool,
) -> fmt::Result {
    let mut s = String::new();

    write!(s, "{}", v.numbers[0])?;
    for n in &v.numbers[1..] {
        write!(s, ".{n}")?;
    }

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

impl<S: Stringable> PartialEq for Version<S> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<S: Stringable> Eq for Version<S> {}

impl<S: Stringable> Hash for Version<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.numbers[0].hash(state);
        for n in &self.numbers[1..] {
            if n.as_ref().starts_with('0') {
                n.as_ref().trim_end_matches('0').hash(state);
            } else {
                n.value.hash(state);
            }
        }
        self.letter.hash(state);
        self.suffixes.hash(state);
        self.revision.hash(state);
    }
}

fn ver_cmp<S: Stringable>(v1: &Version<S>, v2: &Version<S>, rev: bool, op: bool) -> Ordering {
    // compare major versions
    cmp_not_equal!(&v1.numbers[0], &v2.numbers[0]);

    // compare remaining version components
    let mut v1_numbers = v1.numbers[1..].iter();
    let mut v2_numbers = v2.numbers[1..].iter();
    loop {
        match (v1_numbers.next(), v2_numbers.next()) {
            // compare as strings if a component starts with "0"
            (Some(n1), Some(n2))
                if n1.as_ref().starts_with('0') || n2.as_ref().starts_with('0') =>
            {
                cmp_not_equal!(n1.as_ref().trim_end_matches('0'), n2.as_ref().trim_end_matches('0'))
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

impl<S: Stringable> Ord for Version<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        ver_cmp(self, other, true, true)
    }
}

impl<S: Stringable> PartialOrd for Version<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Version<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::new(s)
    }
}

/// Version wrapper that ignores revisions and operators during comparisons.
struct NonRevisionVersion<'a, S: Stringable>(&'a Version<S>);

impl<S: Stringable> PartialEq for NonRevisionVersion<'_, S> {
    fn eq(&self, other: &Self) -> bool {
        ver_cmp(self.0, other.0, false, false) == Ordering::Equal
    }
}

impl<S: Stringable> Eq for NonRevisionVersion<'_, S> {}

impl<S: Stringable> fmt::Display for NonRevisionVersion<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        ver_str(f, self.0, false, false)
    }
}

/// Version wrapper that ignores operators during comparisons.
struct NonOpVersion<'a, S: Stringable>(&'a Version<S>);

impl<S: Stringable> PartialEq for NonOpVersion<'_, S> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<S: Stringable> Eq for NonOpVersion<'_, S> {}

impl<S: Stringable> Ord for NonOpVersion<'_, S> {
    fn cmp(&self, other: &Self) -> Ordering {
        ver_cmp(self.0, other.0, true, false)
    }
}

impl<S: Stringable> PartialOrd for NonOpVersion<'_, S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: Stringable> fmt::Display for NonOpVersion<'_, S> {
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
            assert_eq!(result.unwrap().to_string(), s.as_str());
        }

        // creation forcing with and without operators
        assert!(Version::new_with_op(">1").is_ok());
        assert!(Version::new_with_op("1").is_err());
        assert!(Version::new_without_op(">1").is_err());
        assert!(Version::new_without_op("1").is_ok());
    }

    #[test]
    fn ver_op() {
        let mut ver = Version::new("1").unwrap();
        assert!(ver.op().is_none());
        for op in Operator::iter() {
            ver.with_op(op);
            assert_eq!(ver.op(), Some(op));
        }
    }

    #[test]
    fn rev_new_and_parse() {
        // invalid
        for s in ["a", "a1", "1.1", ".1"] {
            assert!(s.parse::<Revision<String>>().is_err());
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
            let v1: Version<_> = s1.parse().unwrap();
            let v2: Version<_> = s2.parse().unwrap();
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
                let v1: Version<_> = s1.parse().unwrap();
                let v2: Version<_> = s2.parse().unwrap();

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
            let mut reversed: Vec<Version<_>> =
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
            let set: HashSet<Version<_>> = d.versions.iter().map(|s| s.parse().unwrap()).collect();
            if d.equal {
                assert_eq!(set.len(), 1, "failed hashing versions: {set:?}");
            } else {
                assert_eq!(set.len(), d.versions.len(), "failed hashing versions: {set:?}");
            }
        }
    }
}
