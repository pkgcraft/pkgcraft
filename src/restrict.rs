use std::cmp::Ordering;
use std::fmt;
use std::ops::{BitAnd, BitOr, BitXor, Not};

use crate::set::{Ordered, OrderedSet};
use crate::{atom, pkg, Error};

pub mod parse;
pub mod str;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict {
    // boolean
    True,
    False,

    // boolean combinations
    And(Vec<Box<Self>>),
    Or(Vec<Box<Self>>),
    Xor(Vec<Box<Self>>),
    Not(Box<Self>),

    // object attributes
    Atom(atom::Restrict),
    Pkg(pkg::Restrict),

    // strings
    Str(str::Restrict),
}

macro_rules! restrict_match {
   ($r:expr, $obj:expr, $($matcher:pat $(if $pred:expr)* => $result:expr,)+) => {
       match $r {
           $($matcher $(if $pred)* => $result,)+

            // boolean
            Self::True => true,
            Self::False => false,

            // boolean combinations
            Self::And(vals) => vals.iter().all(|r| r.matches($obj)),
            Self::Or(vals) => vals.iter().any(|r| r.matches($obj)),
            Self::Xor(vals) => {
                let mut curr: Option<bool>;
                let mut prev: Option<bool> = None;
                for r in vals.iter() {
                    curr = Some(r.matches($obj));
                    if prev.is_some() && curr != prev {
                        return true;
                    }
                    prev = curr
                }
                false
            },
            Self::Not(r) => !r.matches($obj),

            _ => {
                tracing::warn!("invalid restriction {:?} for matching {:?}", $r, $obj);
                false
            }
       }
   }
}
pub(crate) use restrict_match;

/// Create a restriction enum injected with boolean variants.
macro_rules! restrict_with_boolean {
   ($name:ident, $($variants:tt)*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variants)*
            And(Vec<Box<Self>>),
            Or(Vec<Box<Self>>),
            Xor(Vec<Box<Self>>),
            Not(Box<Self>),
        }
   }
}
pub(crate) use restrict_with_boolean;

/// Implement restriction matching for a type with injected boolean variants.
macro_rules! restrict_match_boolean {
   ($r:expr, $obj:expr, $($matcher:pat $(if $pred:expr)* => $result:expr,)+) => {
       match $r {
           $($matcher $(if $pred)* => $result,)+
            Self::And(vals) => vals.iter().all(|r| r.matches($obj)),
            Self::Or(vals) => vals.iter().any(|r| r.matches($obj)),
            Self::Xor(vals) => {
                let mut curr: Option<bool>;
                let mut prev: Option<bool> = None;
                for r in vals.iter() {
                    curr = Some(r.matches($obj));
                    if prev.is_some() && curr != prev {
                        return true;
                    }
                    prev = curr
                }
                false
            },
            Self::Not(r) => !r.matches($obj),
       }
   }
}
pub(crate) use restrict_match_boolean;

/// Implement boolean restriction conversions for injected boolean variants.
macro_rules! restrict_impl_boolean {
    ($type:ty) => {
        pub fn and<I, T>(iter: I) -> Self
        where
            I: IntoIterator<Item = T>,
            T: Into<$type>,
        {
            let mut restricts = vec![];
            for r in iter.into_iter().map(Into::into) {
                match r {
                    Self::And(vals) => restricts.extend(vals),
                    _ => restricts.push(Box::new(r)),
                }
            }
            Self::And(restricts)
        }

        pub fn or<I, T>(iter: I) -> Self
        where
            I: IntoIterator<Item = T>,
            T: Into<$type>,
        {
            let mut restricts = vec![];
            for r in iter.into_iter().map(Into::into) {
                match r {
                    Self::Or(vals) => restricts.extend(vals),
                    _ => restricts.push(Box::new(r)),
                }
            }
            Self::Or(restricts)
        }

        pub fn xor<I, T>(iter: I) -> Self
        where
            I: IntoIterator<Item = T>,
            T: Into<$type>,
        {
            let mut restricts = vec![];
            for r in iter.into_iter().map(Into::into) {
                match r {
                    Self::Xor(vals) => restricts.extend(vals),
                    _ => restricts.push(Box::new(r)),
                }
            }
            Self::Xor(restricts)
        }

        pub fn not<T>(obj: T) -> Self
        where
            T: Into<$type>,
        {
            Self::Not(Box::new(obj.into()))
        }
    };
}
pub(crate) use restrict_impl_boolean;

impl Restrict {
    restrict_impl_boolean! {Self}
}

impl BitAnd for Restrict {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::and([self, rhs])
    }
}

impl BitOr for Restrict {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::or([self, rhs])
    }
}

impl BitXor for Restrict {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self::xor([self, rhs])
    }
}

impl Not for Restrict {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::Not(Box::new(self))
    }
}

pub trait Restriction<T>: fmt::Debug {
    fn matches(&self, object: T) -> bool;
}

impl Restriction<&str> for Restrict {
    fn matches(&self, s: &str) -> bool {
        restrict_match! {self, s,
            Self::Str(r) => r.matches(s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OrderedSetRestrict<T: Ordered, R> {
    Empty,
    Contains(T),
    Disjoint(OrderedSet<T>),
    Equal(OrderedSet<T>),
    Subset(OrderedSet<T>),
    ProperSubset(OrderedSet<T>),
    Superset(OrderedSet<T>),
    ProperSuperset(OrderedSet<T>),
    Ordered(OrderedRestrict<R>),
}

macro_rules! make_set_restriction {
    ($(($container:ty, $element:ty, $restrict:ty)),+) => {$(
        impl Restriction<&$container> for OrderedSetRestrict<$element, $restrict> {
            fn matches(&self, val: &$container) -> bool {
                match self {
                    Self::Empty => val.is_empty(),
                    Self::Contains(s) => val.contains(s),
                    Self::Disjoint(s) => val.is_disjoint(s),
                    Self::Equal(s) => val == s,
                    Self::Subset(s) => val.is_subset(s),
                    Self::ProperSubset(s) => val.is_subset(s) && val != s,
                    Self::Superset(s) => val.is_superset(s),
                    Self::ProperSuperset(s) => val.is_superset(s) && val != s,
                    Self::Ordered(r) => r.matches(val),
                }
            }
        }
    )+};
}
pub(crate) use make_set_restriction;
make_set_restriction!((OrderedSet<String>, String, str::Restrict));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OrderedRestrict<R> {
    Any(R),
    All(R),
    First(R),
    Last(R),
    Count(Vec<Ordering>, usize),
}

macro_rules! make_ordered_restrictions {
    ($(($x:ty, $r:ty)),+) => {$(
        impl Restriction<$x> for OrderedRestrict<$r> {
            fn matches(&self, val: $x) -> bool {
                match self {
                    Self::Any(r) => val.iter().any(|v| r.matches(v)),
                    Self::All(r) => val.iter().all(|v| r.matches(v)),
                    Self::First(r) => val.first().map(|v| r.matches(v)).unwrap_or_default(),
                    Self::Last(r) => val.last().map(|v| r.matches(v)).unwrap_or_default(),
                    Self::Count(ordering, size) => ordering.contains(&val.len().cmp(size)),
                }
            }
        }
    )+};
}
pub(crate) use make_ordered_restrictions;
make_ordered_restrictions!((&[String], str::Restrict), (&OrderedSet<String>, str::Restrict));

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::Atom;

    use super::*;

    #[test]
    fn test_filtering() {
        let atom_strs = vec!["cat/pkg", ">=cat/pkg-1", "=cat/pkg-1:2/3::repo"];
        let atoms: Vec<Atom> = atom_strs
            .iter()
            .map(|s| Atom::from_str(s).unwrap())
            .collect();

        let filter = |r: Restrict, atoms: Vec<Atom>| -> Vec<String> {
            atoms
                .into_iter()
                .filter(|a| r.matches(a))
                .map(|a| a.to_string())
                .collect()
        };

        let r = Restrict::Atom(atom::Restrict::category("cat"));
        assert_eq!(filter(r, atoms.clone()), atom_strs);

        let r = Restrict::Atom(atom::Restrict::Version(None));
        assert_eq!(filter(r, atoms.clone()), ["cat/pkg"]);

        let cpv = Atom::from_str("=cat/pkg-1").unwrap();
        let r = Restrict::from(&cpv);
        assert_eq!(filter(r, atoms.clone()), [">=cat/pkg-1", "=cat/pkg-1:2/3::repo"]);

        let r = Restrict::True;
        assert_eq!(filter(r, atoms.clone()), atom_strs);

        let r = Restrict::False;
        assert!(filter(r, atoms).is_empty());
    }

    #[test]
    fn test_and_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkg");
        let r = Restrict::and([cat, pkg]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkga");
        let r = Restrict::and([cat, pkg]);
        assert!(!(r.matches(&a)));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = Restrict::and([&a1, &a2]);
        assert!(!(r.matches(&a1)));
        assert!(!(r.matches(&a2)));
    }

    #[test]
    fn test_or_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkg");
        let r = Restrict::or([cat, pkg]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkga");
        let r = Restrict::or([cat, pkg]);
        assert!(r.matches(&a));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = Restrict::or([&a1, &a2]);
        assert!(r.matches(&a1));
        assert!(r.matches(&a2));
    }

    #[test]
    fn test_xor_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();

        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkg");
        let nover = atom::Restrict::Version(None);

        // two matches
        let r = Restrict::xor([cat.clone(), pkg.clone()]);
        assert!(!(r.matches(&a)));

        // three matches
        let r = Restrict::xor([cat, pkg, nover.clone()]);
        assert!(!(r.matches(&a)));

        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkga");
        let ver = atom::Restrict::version("1").unwrap();

        // one matched and one unmatched
        let r = Restrict::xor([cat.clone(), pkg.clone()]);
        assert!(r.matches(&a));

        // one matched and two unmatched
        let r = Restrict::xor([cat.clone(), pkg.clone(), ver]);
        assert!(r.matches(&a));

        // two matched and one unmatched
        let r = Restrict::xor([cat, pkg, nover]);
        assert!(r.matches(&a));

        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let a3 = Atom::from_str("cat/pkg3").unwrap();

        // two non-matches
        let r = Restrict::xor([&a1, &a2]);
        assert!(!(r.matches(&a)));

        // three non-matches
        let r = Restrict::xor([&a1, &a2, &a3]);
        assert!(!(r.matches(&a)));
    }

    #[test]
    fn test_not_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let r: Restrict = atom::Restrict::category("cat1").into();

        // restrict doesn't match
        assert!(!(r.matches(&a)));

        // inverse matches
        assert!(!r.matches(&a));
    }
}
