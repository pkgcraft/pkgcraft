use std::collections::VecDeque;
use std::fmt;

use itertools::Itertools;

use crate::dep::PkgDep;
use crate::macros::extend_left;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::set::{Ordered, OrderedSet, SortedSet};

/// Uri object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uri {
    pub(crate) uri: String,
    pub(crate) rename: Option<String>,
}

impl Uri {
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn rename(&self) -> Option<&str> {
        self.rename.as_deref()
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.uri)?;
        if let Some(s) = &self.rename {
            write!(f, " -> {s}")?;
        }
        Ok(())
    }
}

impl AsRef<str> for Uri {
    fn as_ref(&self) -> &str {
        &self.uri
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DepSet<T: Ordered>(SortedSet<Dep<T>>);

impl<T: Ordered> Default for DepSet<T> {
    fn default() -> Self {
        Self(SortedSet::new())
    }
}

macro_rules! p {
    ($x:expr) => {
        $x.into_iter().map(|x| x.to_string()).join(" ")
    };
}

impl<T: fmt::Display + Ordered> fmt::Display for DepSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", p!(&self.0))
    }
}

impl<T: Ordered> FromIterator<Dep<T>> for DepSet<T> {
    fn from_iter<I: IntoIterator<Item = Dep<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

pub trait IntoIteratorDepSet {
    type FlattenedItem;
    type IntoIterFlatten: Iterator<Item = Self::FlattenedItem>;

    type RecursiveItem;
    type IntoIterRecursive: Iterator<Item = Self::RecursiveItem>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten;
    fn into_iter_recursive(self) -> Self::IntoIterRecursive;
}

impl<T: Ordered> DepSet<T> {
    pub fn iter(&self) -> DepSetIter<T> {
        self.into_iter()
    }

    pub fn iter_flatten(&self) -> DepSetIterFlatten<T> {
        self.into_iter_flatten()
    }
}

#[derive(Debug)]
pub struct DepSetIter<'a, T: Ordered>(indexmap::set::Iter<'a, Dep<T>>);

impl<'a, T: Ordered> IntoIterator for &'a DepSet<T> {
    type Item = &'a Dep<T>;
    type IntoIter = DepSetIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        DepSetIter(self.0.iter())
    }
}

impl<'a, T: Ordered> Iterator for DepSetIter<'a, T> {
    type Item = &'a Dep<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T: Ordered> IntoIteratorDepSet for &'a DepSet<T> {
    type FlattenedItem = &'a T;
    type IntoIterFlatten = DepSetIterFlatten<'a, T>;

    type RecursiveItem = &'a Dep<T>;
    type IntoIterRecursive = DepSetIterRecursive<'a, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        DepSetIterFlatten(self.0.iter().collect())
    }

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        DepSetIterRecursive(self.0.iter().collect())
    }
}

#[derive(Debug)]
pub struct DepSetIntoIter<T: Ordered>(indexmap::set::IntoIter<Dep<T>>);

impl<T: Ordered> IntoIterator for DepSet<T> {
    type Item = Dep<T>;
    type IntoIter = DepSetIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        DepSetIntoIter(self.0.into_iter())
    }
}

impl<T: Ordered> Iterator for DepSetIntoIter<T> {
    type Item = Dep<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<T: Ordered> IntoIteratorDepSet for DepSet<T> {
    type FlattenedItem = T;
    type IntoIterFlatten = DepSetIntoIterFlatten<T>;

    type RecursiveItem = Dep<T>;
    type IntoIterRecursive = DepSetIntoIterRecursive<T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        DepSetIntoIterFlatten(self.0.into_iter().collect())
    }

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        DepSetIntoIterRecursive(self.0.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dep<T: Ordered> {
    Enabled(T),
    Disabled(T), // REQUIRED_USE only
    AllOf(SortedSet<Box<Dep<T>>>),
    AnyOf(OrderedSet<Box<Dep<T>>>),
    ExactlyOneOf(OrderedSet<Box<Dep<T>>>), // REQUIRED_USE only
    AtMostOneOf(OrderedSet<Box<Dep<T>>>),  // REQUIRED_USE only
    UseEnabled(String, SortedSet<Box<Dep<T>>>),
    UseDisabled(String, SortedSet<Box<Dep<T>>>),
}

impl<T: Ordered> Dep<T> {
    pub fn iter_flatten(&self) -> DepSetIterFlatten<T> {
        self.into_iter_flatten()
    }
}

impl<'a, T: Ordered> IntoIteratorDepSet for &'a Dep<T> {
    type FlattenedItem = &'a T;
    type IntoIterFlatten = DepSetIterFlatten<'a, T>;

    type RecursiveItem = &'a Dep<T>;
    type IntoIterRecursive = DepSetIterRecursive<'a, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        DepSetIterFlatten([self].into_iter().collect())
    }

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        DepSetIterRecursive([self].into_iter().collect())
    }
}

impl<T: Ordered> IntoIteratorDepSet for Dep<T> {
    type FlattenedItem = T;
    type IntoIterFlatten = DepSetIntoIterFlatten<T>;

    type RecursiveItem = Dep<T>;
    type IntoIterRecursive = DepSetIntoIterRecursive<T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        DepSetIntoIterFlatten([self].into_iter().collect())
    }

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        DepSetIntoIterRecursive([self].into_iter().collect())
    }
}

impl<T: fmt::Display + Ordered> fmt::Display for Dep<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Dep::*;
        match self {
            Enabled(val) => write!(f, "{val}"),
            Disabled(val) => write!(f, "!{val}"),
            AllOf(vals) => write!(f, "( {} )", p!(vals)),
            AnyOf(vals) => write!(f, "|| ( {} )", p!(vals)),
            ExactlyOneOf(vals) => write!(f, "^^ ( {} )", p!(vals)),
            AtMostOneOf(vals) => write!(f, "?? ( {} )", p!(vals)),
            UseEnabled(s, vals) => write!(f, "{s}? ( {} )", p!(vals)),
            UseDisabled(s, vals) => write!(f, "!{s}? ( {} )", p!(vals)),
        }
    }
}

#[derive(Debug)]
pub struct DepSetIterFlatten<'a, T: Ordered>(VecDeque<&'a Dep<T>>);

impl<'a, T: fmt::Debug + Ordered> Iterator for DepSetIterFlatten<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        use Dep::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                UseEnabled(_, vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                UseDisabled(_, vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct DepSetIntoIterFlatten<T: Ordered>(VecDeque<Dep<T>>);

impl<T: fmt::Debug + Ordered> Iterator for DepSetIntoIterFlatten<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        use Dep::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => extend_left!(self.0, vals.into_iter().map(|x| *x)),
                AnyOf(vals) => extend_left!(self.0, vals.into_iter().map(|x| *x)),
                ExactlyOneOf(vals) => extend_left!(self.0, vals.into_iter().map(|x| *x)),
                AtMostOneOf(vals) => extend_left!(self.0, vals.into_iter().map(|x| *x)),
                UseEnabled(_, vals) => extend_left!(self.0, vals.into_iter().map(|x| *x)),
                UseDisabled(_, vals) => extend_left!(self.0, vals.into_iter().map(|x| *x)),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct DepSetIterRecursive<'a, T: Ordered>(VecDeque<&'a Dep<T>>);

impl<'a, T: fmt::Debug + Ordered> Iterator for DepSetIterRecursive<'a, T> {
    type Item = &'a Dep<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dep::*;
        if let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                UseEnabled(_, vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
                UseDisabled(_, vals) => extend_left!(self.0, vals.iter().map(AsRef::as_ref)),
            }
            Some(dep)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct DepSetIntoIterRecursive<T: Ordered>(VecDeque<Dep<T>>);

impl<T: fmt::Debug + Ordered> Iterator for DepSetIntoIterRecursive<T> {
    type Item = Dep<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dep::*;
        if let Some(dep) = self.0.pop_front() {
            match &dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => extend_left!(self.0, vals.into_iter().map(|x| *x.clone())),
                AnyOf(vals) => extend_left!(self.0, vals.into_iter().map(|x| *x.clone())),
                ExactlyOneOf(vals) => extend_left!(self.0, vals.into_iter().map(|x| *x.clone())),
                AtMostOneOf(vals) => extend_left!(self.0, vals.into_iter().map(|x| *x.clone())),
                UseEnabled(_, vals) => extend_left!(self.0, vals.into_iter().map(|x| *x.clone())),
                UseDisabled(_, vals) => extend_left!(self.0, vals.into_iter().map(|x| *x.clone())),
            }
            Some(dep)
        } else {
            None
        }
    }
}

impl Restriction<&DepSet<PkgDep>> for BaseRestrict {
    fn matches(&self, val: &DepSet<PkgDep>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&Dep<PkgDep>> for BaseRestrict {
    fn matches(&self, val: &Dep<PkgDep>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}
