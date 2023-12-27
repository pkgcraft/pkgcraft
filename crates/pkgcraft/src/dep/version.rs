use std::cmp::Ordering;
use std::fmt::{self, Write};
use std::hash::{Hash, Hasher};
use std::str;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};

use crate::macros::{
    cmp_not_equal, equivalent, partial_cmp_not_equal, partial_cmp_not_equal_opt, partial_cmp_opt,
    partial_eq_opt,
};
use crate::traits::{Intersects, IntoOwned, ToRef};
use crate::Error;

use super::{parse, Stringable};

/// Modify or create a new type by adding a version operator.
pub trait WithOp {
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

impl<'a> ToRef<'a> for Number<String> {
    type Ref = Number<&'a str>;

    fn to_ref(&'a self) -> Self::Ref {
        Number {
            raw: self.raw.as_ref(),
            value: self.value,
        }
    }
}

impl<S: Stringable> Number<S> {
    /// Determine if a number is represented by an empty string.
    pub(crate) fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }

    /// Return the raw string value for a number.
    pub(crate) fn as_str(&self) -> &str {
        self.raw.as_ref()
    }

    /// Determine if a Number starts with another Number using string representation.
    fn starts_with<T: Stringable>(&self, other: &Number<T>) -> bool {
        self.as_str().starts_with(other.as_str())
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Number<S1>> for Number<S2> {
    fn eq(&self, other: &Number<S1>) -> bool {
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
        self.as_str() == other
    }
}

impl<S: Stringable> PartialEq<&str> for Number<S> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
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

impl<S1: Stringable, S2: Stringable> PartialOrd<Number<S1>> for Number<S2> {
    fn partial_cmp(&self, other: &Number<S1>) -> Option<Ordering> {
        Some(self.value.cmp(&other.value))
    }
}

impl<S: Stringable> fmt::Display for Number<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Eq, Ord, Hash, Clone)]
pub struct Revision<S: Stringable>(pub(crate) Number<S>);

impl IntoOwned for Revision<&str> {
    type Owned = Revision<String>;

    fn into_owned(self) -> Self::Owned {
        Revision(self.0.into_owned())
    }
}

impl<'a> ToRef<'a> for Revision<String> {
    type Ref = Revision<&'a str>;

    fn to_ref(&'a self) -> Self::Ref {
        Revision(self.0.to_ref())
    }
}

impl Revision<String> {
    /// Create a new Revision from a given string.
    pub fn try_new(s: &str) -> crate::Result<Self> {
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

    /// Return the raw string value for a revision.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Determine if a Revision starts with another Revision using string representation.
    fn starts_with<T: Stringable>(&self, other: &Revision<T>) -> bool {
        self.0.starts_with(&other.0)
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Revision<S1>> for Revision<S2> {
    fn eq(&self, other: &Revision<S1>) -> bool {
        self.0 == other.0
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Revision<S1>> for Revision<S2> {
    fn partial_cmp(&self, other: &Revision<S1>) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<S: Stringable> PartialEq<u64> for Revision<S> {
    fn eq(&self, other: &u64) -> bool {
        &self.0.value == other
    }
}

impl<S: Stringable> PartialEq<str> for Revision<S> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<S: Stringable> PartialEq<&str> for Revision<S> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
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
        Self::try_new(s)
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

#[derive(Debug, Serialize, Deserialize, Eq, Ord, Hash, Clone)]
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

impl<'a> ToRef<'a> for Suffix<String> {
    type Ref = Suffix<&'a str>;

    fn to_ref(&'a self) -> Self::Ref {
        Suffix {
            kind: self.kind,
            version: self.version.as_ref().map(|x| x.to_ref()),
        }
    }
}

impl<S1: Stringable, S2: Stringable> PartialEq<Suffix<S1>> for Suffix<S2> {
    fn eq(&self, other: &Suffix<S1>) -> bool {
        self.kind == other.kind && partial_eq_opt!(&self.version, &other.version)
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Suffix<S1>> for Suffix<S2> {
    fn partial_cmp(&self, other: &Suffix<S1>) -> Option<Ordering> {
        partial_cmp_not_equal_opt!(&self.kind, &other.kind);
        partial_cmp_opt!(&self.version, &other.version)
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
    /// Determine if two versions intersect for an operator.
    fn intersects<S1, S2>(&self, lhs: &Version<S1>, rhs: &Version<S2>) -> bool
    where
        S1: Stringable,
        S2: Stringable,
    {
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

impl<S: Stringable> WithOp for Version<S> {
    type WithOp = Version<S>;

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

impl<'a> ToRef<'a> for Version<String> {
    type Ref = Version<&'a str>;

    fn to_ref(&'a self) -> Self::Ref {
        Version {
            op: self.op,
            numbers: self.numbers.iter().map(|x| x.to_ref()).collect(),
            letter: self.letter,
            suffixes: self.suffixes.iter().map(|x| x.to_ref()).collect(),
            revision: self.revision.to_ref(),
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

    /// Create a borrowed [`Version`] with an [`Operator`].
    pub fn parse_with_op(s: &'a str) -> crate::Result<Self> {
        parse::version_with_op(s)
    }

    /// Create a borrowed [`Version`] without an [`Operator`].
    pub fn parse_without_op(s: &'a str) -> crate::Result<Self> {
        parse::version(s)
    }
}

impl Version<String> {
    /// Create an owned [`Version`] from a given string with or without an [`Operator`].
    pub fn try_new(s: &str) -> crate::Result<Self> {
        Version::parse(s).into_owned()
    }

    /// Create an owned [`Version`] with an [`Operator`].
    pub fn try_new_with_op(s: &str) -> crate::Result<Self> {
        Version::parse_with_op(s).into_owned()
    }

    /// Create an owned [`Version`] without an [`Operator`].
    pub fn try_new_without_op(s: &str) -> crate::Result<Self> {
        Version::parse_without_op(s).into_owned()
    }
}

impl<S: Stringable> Version<S> {
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
    fn starts_with<T: Stringable>(&self, other: &Version<T>) -> bool {
        // flag denoting the lhs has more components than the rhs
        let mut unmatched = false;

        // compare components
        let mut v1_numbers = self.numbers.iter();
        let mut v2_numbers = other.numbers.iter();
        loop {
            match (v1_numbers.next(), v2_numbers.next()) {
                (Some(n1), Some(n2)) => {
                    if !n1.starts_with(n2) {
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
                            if !v1.starts_with(v2) {
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
            (Less | LessOrEqual, EqualGlob) => match $other.revision().map(|r| r.as_str()) {
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

impl<S1: Stringable, S2: Stringable> Intersects<Version<S1>> for Version<S2> {
    fn intersects(&self, other: &Version<S1>) -> bool {
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
        fmt(f, self, true, true)
    }
}

/// Format a [`Version`] into a [`String`], optionally ignoring the revision and/or operator.
fn fmt<S: Stringable>(f: &mut fmt::Formatter, v: &Version<S>, rev: bool, op: bool) -> fmt::Result {
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

impl<S1: Stringable, S2: Stringable> PartialEq<Version<S1>> for Version<S2> {
    fn eq(&self, other: &Version<S1>) -> bool {
        cmp(self, other, true, true) == Ordering::Equal
    }
}

impl<S: Stringable> Eq for Version<S> {}

impl<S: Stringable> Hash for Version<S> {
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
fn cmp<S1, S2>(v1: &Version<S1>, v2: &Version<S2>, rev: bool, op: bool) -> Ordering
where
    S1: Stringable,
    S2: Stringable,
{
    // compare major versions
    partial_cmp_not_equal!(&v1.numbers[0], &v2.numbers[0]);

    // compare remaining version components
    let mut v1_numbers = v1.numbers[1..].iter();
    let mut v2_numbers = v2.numbers[1..].iter();
    loop {
        match (v1_numbers.next(), v2_numbers.next()) {
            (Some(n1), Some(n2)) => {
                // compare as strings if a component starts with "0", otherwise as integers
                let (s1, s2) = (n1.as_str(), n2.as_str());
                if s1.starts_with('0') || s2.starts_with('0') {
                    cmp_not_equal!(s1.trim_end_matches('0'), s2.trim_end_matches('0'));
                } else {
                    partial_cmp_not_equal!(n1, n2);
                }
            }
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
            (Some(s1), Some(s2)) => partial_cmp_not_equal!(s1, s2),
            (Some(Suffix { kind: SuffixKind::P, .. }), None) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (None, Some(Suffix { kind: SuffixKind::P, .. })) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => break,
        }
    }

    // compare revisions
    if rev {
        partial_cmp_not_equal!(&v1.revision, &v2.revision);
    }

    // compare operators
    if op {
        cmp_not_equal!(&v1.op, &v2.op);
    }

    Ordering::Equal
}

impl<S: Stringable> Ord for Version<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp(self, other, true, true)
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<Version<S1>> for Version<S2> {
    fn partial_cmp(&self, other: &Version<S1>) -> Option<Ordering> {
        Some(cmp(self, other, true, true))
    }
}

impl FromStr for Version<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Self::try_new(s)
    }
}

equivalent!(Version);

/// Version wrapper that ignores revisions and operators during comparisons.
struct NonRevisionVersion<'a, S: Stringable>(&'a Version<S>);

impl<S1: Stringable, S2: Stringable> PartialEq<NonRevisionVersion<'_, S1>>
    for NonRevisionVersion<'_, S2>
{
    fn eq(&self, other: &NonRevisionVersion<'_, S1>) -> bool {
        cmp(self.0, other.0, false, false) == Ordering::Equal
    }
}

impl<S: Stringable> Eq for NonRevisionVersion<'_, S> {}

impl<S: Stringable> fmt::Display for NonRevisionVersion<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt(f, self.0, false, false)
    }
}

/// Version wrapper that ignores operators during comparisons.
struct NonOpVersion<'a, S: Stringable>(&'a Version<S>);

impl<S1: Stringable, S2: Stringable> PartialEq<NonOpVersion<'_, S1>> for NonOpVersion<'_, S2> {
    fn eq(&self, other: &NonOpVersion<'_, S1>) -> bool {
        cmp(self.0, other.0, true, false) == Ordering::Equal
    }
}

impl<S: Stringable> Eq for NonOpVersion<'_, S> {}

impl<S: Stringable> Ord for NonOpVersion<'_, S> {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp(self.0, other.0, true, false)
    }
}

impl<S1: Stringable, S2: Stringable> PartialOrd<NonOpVersion<'_, S1>> for NonOpVersion<'_, S2> {
    fn partial_cmp(&self, other: &NonOpVersion<'_, S1>) -> Option<Ordering> {
        Some(cmp(self.0, other.0, true, false))
    }
}

impl<S: Stringable> fmt::Display for NonOpVersion<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt(f, self.0, true, false)
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
    fn ver_new_and_parse() {
        // invalid
        for s in &TEST_DATA.version_toml.invalid {
            let result = Version::parse(s);
            assert!(result.is_err(), "{s:?} is valid");
            let result = Version::try_new(s);
            assert!(result.is_err(), "{s:?} didn't fail");
        }

        // valid
        for s in &TEST_DATA.version_toml.valid {
            let result = Version::parse(s);
            assert!(result.is_ok(), "{s:?} is invalid");
            let result = Version::try_new(s);
            assert!(result.is_ok(), "{s:?} failed");
            assert_eq!(result.unwrap().to_string(), s.as_str());
        }

        // forced with and without operators
        assert!(Version::parse_with_op(">1").is_ok());
        assert!(Version::try_new_with_op(">1").is_ok());
        assert!(Version::parse_with_op("1").is_err());
        assert!(Version::try_new_with_op("1").is_err());
        assert!(Version::parse_without_op(">1").is_err());
        assert!(Version::try_new_without_op(">1").is_err());
        assert!(Version::parse_without_op("1").is_ok());
        assert!(Version::try_new_without_op("1").is_ok());
    }

    #[test]
    fn ver_op() {
        let ver = Version::try_new("1").unwrap();
        assert!(ver.op().is_none());
        let ver = Version::parse("1").unwrap();
        assert!(ver.op().is_none());

        for op in Operator::iter() {
            let ver = Version::try_new("1").unwrap().with_op(op).unwrap();
            assert_eq!(ver.op(), Some(op));
            let ver = Version::parse("1").unwrap().with_op(op).unwrap();
            assert_eq!(ver.op(), Some(op));
        }
    }

    #[test]
    fn rev_new_and_parse() {
        // invalid
        for s in ["a", "a1", "1.1", ".1"] {
            assert!(s.parse::<Revision<_>>().is_err());
            assert!(Revision::try_new(s).is_err());
        }

        // empty
        let rev = Revision::try_new("").unwrap();
        assert_eq!(rev, 0);
        assert_eq!(0, rev);
        assert_eq!(rev, "");
        assert_eq!("", rev);
        assert_eq!(rev, Revision::<String>::default());
        assert_eq!(rev.to_string(), "");

        // simple integer
        let rev1 = Revision::try_new("1").unwrap();
        assert_eq!(rev1, 1);
        assert_eq!(1, rev1);
        assert_eq!(rev1, "1");
        assert_eq!("1", rev1);
        assert_eq!(rev1.to_string(), "1");

        // zero prefixes are technically allowed
        let rev2 = Revision::try_new("01").unwrap();
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
            let v1_owned = Version::try_new(s1).unwrap();
            let v1_borrowed = Version::parse(s1).unwrap();
            let v2_owned = Version::try_new(s2).unwrap();
            let v2_borrowed = Version::parse(s2).unwrap();
            if op == "!=" {
                assert_ne!(v1_owned, v2_owned, "failed comparing: {expr}");
                assert_ne!(v1_borrowed, v2_borrowed, "failed comparing: {expr}");
                assert_ne!(v1_owned, v2_borrowed, "failed comparing: {expr}");
                assert_ne!(v1_borrowed, v2_owned, "failed comparing: {expr}");
                assert_ne!(v2_owned, v1_owned, "failed comparing: {expr}");
                assert_ne!(v2_borrowed, v1_borrowed, "failed comparing: {expr}");
                assert_ne!(v2_owned, v1_borrowed, "failed comparing: {expr}");
                assert_ne!(v2_borrowed, v1_owned, "failed comparing: {expr}");
            } else {
                let op = op_map[op];
                assert_eq!(v1_owned.cmp(&v2_owned), op, "failed comparing: {expr}");
                assert_eq!(v1_borrowed.cmp(&v2_borrowed), op, "failed comparing: {expr}");
                assert_eq!(
                    v1_owned.partial_cmp(&v2_borrowed),
                    Some(op),
                    "failed comparing: {expr}"
                );
                assert_eq!(
                    v1_borrowed.partial_cmp(&v2_owned),
                    Some(op),
                    "failed comparing: {expr}"
                );
                assert_eq!(
                    v2_owned.cmp(&v1_owned),
                    op.reverse(),
                    "failed comparing inverted: {expr}"
                );
                assert_eq!(
                    v2_borrowed.cmp(&v1_borrowed),
                    op.reverse(),
                    "failed comparing inverted: {expr}"
                );
                assert_eq!(
                    v2_owned.partial_cmp(&v1_borrowed),
                    Some(op.reverse()),
                    "failed comparing inverted: {expr}"
                );
                assert_eq!(
                    v2_borrowed.partial_cmp(&v1_owned),
                    Some(op.reverse()),
                    "failed comparing inverted: {expr}"
                );

                // verify the following property holds since both Hash and Eq are implemented:
                // k1 == k2 -> hash(k1) == hash(k2)
                if op == Ordering::Equal {
                    assert_eq!(hash(&v1_owned), hash(&v2_owned), "failed hash: {expr}");
                    assert_eq!(hash(&v1_borrowed), hash(&v2_borrowed), "failed hash: {expr}");
                    assert_eq!(hash(&v1_owned), hash(&v2_borrowed), "failed hash: {expr}");
                    assert_eq!(hash(&v1_borrowed), hash(&v2_owned), "failed hash: {expr}");
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
                let v1_owned = Version::try_new(s1).unwrap();
                let v1_borrowed = Version::parse(s1).unwrap();
                let v2_owned = Version::try_new(s2).unwrap();
                let v2_borrowed = Version::parse(s2).unwrap();

                // self intersection
                assert!(v1_owned.intersects(&v1_owned), "{s1} doesn't intersect {s2}");
                assert!(v1_borrowed.intersects(&v1_borrowed), "{s1} doesn't intersect {s2}");
                assert!(v1_owned.intersects(&v1_borrowed), "{s1} doesn't intersect {s2}");
                assert!(v1_borrowed.intersects(&v1_owned), "{s1} doesn't intersect {s2}");
                assert!(v2_owned.intersects(&v2_owned), "{s2} doesn't intersect {s2}");
                assert!(v2_borrowed.intersects(&v2_borrowed), "{s2} doesn't intersect {s2}");
                assert!(v2_owned.intersects(&v2_borrowed), "{s2} doesn't intersect {s2}");
                assert!(v2_borrowed.intersects(&v2_owned), "{s2} doesn't intersect {s2}");

                // intersects depending on status
                if d.status {
                    assert!(v1_owned.intersects(&v2_owned), "{s1} doesn't intersect {s2}");
                    assert!(v1_borrowed.intersects(&v2_borrowed), "{s1} doesn't intersect {s2}");
                    assert!(v1_owned.intersects(&v2_borrowed), "{s1} doesn't intersect {s2}");
                    assert!(v1_borrowed.intersects(&v2_owned), "{s1} doesn't intersect {s2}");
                } else {
                    assert!(!v1_owned.intersects(&v2_owned), "{s1} intersects {s2}");
                    assert!(!v1_borrowed.intersects(&v2_borrowed), "{s1} intersects {s2}");
                    assert!(!v1_owned.intersects(&v2_borrowed), "{s1} intersects {s2}");
                    assert!(!v1_borrowed.intersects(&v2_owned), "{s1} intersects {s2}");
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
