use std::collections::VecDeque;
use std::fmt;

use itertools::Itertools;

use crate::macros::extend_left;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::types::{Ordered, OrderedSet, SortedSet};
use crate::Error;

use super::Dep;

/// Flattened iterator support for dependency objects.
pub trait Flatten {
    type Item;
    type IntoIterFlatten: Iterator<Item = Self::Item>;
    fn into_iter_flatten(self) -> Self::IntoIterFlatten;
}

/// Recursive iterator support for dependency objects.
pub trait Recursive {
    type Item;
    type IntoIterRecursive: Iterator<Item = Self::Item>;
    fn into_iter_recursive(self) -> Self::IntoIterRecursive;
}

/// Uri object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uri {
    uri: String,
    filename: String,
    rename: bool,
}

impl Uri {
    pub(crate) fn new(uri: &str, rename: Option<&str>) -> crate::Result<Self> {
        let uri = uri.trim();
        let filename = rename.unwrap_or_else(|| match uri.rsplit_once('/') {
            Some((_, filename)) => filename,
            None => uri,
        });

        // rudimentary URI validity check since full parsing isn't used
        if filename.is_empty() {
            return Err(Error::InvalidValue(format!("URI missing filename: {uri}")));
        }

        Ok(Self {
            uri: uri.to_string(),
            filename: filename.to_string(),
            rename: rename.is_some(),
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.uri)?;
        if self.rename {
            write!(f, " -> {}", self.filename)?;
        }
        Ok(())
    }
}

impl AsRef<str> for Uri {
    fn as_ref(&self) -> &str {
        &self.uri
    }
}

macro_rules! p {
    ($x:expr) => {
        $x.into_iter().map(|x| x.to_string()).join(" ")
    };
}

/// Dependency specification variants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DepSpec<T: Ordered> {
    /// Enabled dependency.
    Enabled(T),
    /// Disabled dependency.
    Disabled(T), // REQUIRED_USE only
    /// All of a given dependency set.
    AllOf(SortedSet<Box<DepSpec<T>>>),
    /// Any of a given dependency set.
    AnyOf(OrderedSet<Box<DepSpec<T>>>),
    /// Exactly one of a given dependency set (REQUIRED_USE only).
    ExactlyOneOf(OrderedSet<Box<DepSpec<T>>>),
    /// At most of a given dependency set (REQUIRED_USE only).
    AtMostOneOf(OrderedSet<Box<DepSpec<T>>>),
    /// Conditionally enabled dependency.
    UseEnabled(String, SortedSet<Box<DepSpec<T>>>),
    /// Conditionally disabled dependency.
    UseDisabled(String, SortedSet<Box<DepSpec<T>>>),
}

impl<T: Ordered> DepSpec<T> {
    pub fn iter_flatten(&self) -> IterFlatten<T> {
        self.into_iter_flatten()
    }
}

impl<'a, T: Ordered> Flatten for &'a DepSpec<T> {
    type Item = &'a T;
    type IntoIterFlatten = IterFlatten<'a, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IterFlatten([self].into_iter().collect())
    }
}

impl<'a, T: Ordered> Recursive for &'a DepSpec<T> {
    type Item = &'a DepSpec<T>;
    type IntoIterRecursive = IterRecursive<'a, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IterRecursive([self].into_iter().collect())
    }
}

impl<T: Ordered> Flatten for DepSpec<T> {
    type Item = T;
    type IntoIterFlatten = IntoIterFlatten<T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IntoIterFlatten([self].into_iter().collect())
    }
}

impl<T: Ordered> Recursive for DepSpec<T> {
    type Item = DepSpec<T>;
    type IntoIterRecursive = IntoIterRecursive<T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IntoIterRecursive([self].into_iter().collect())
    }
}

impl<T: fmt::Display + Ordered> fmt::Display for DepSpec<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use DepSpec::*;
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

/// Set of dependency objects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DepSet<T: Ordered>(SortedSet<DepSpec<T>>);

impl<T: Ordered> Default for DepSet<T> {
    fn default() -> Self {
        Self(SortedSet::new())
    }
}

impl<T: fmt::Display + Ordered> fmt::Display for DepSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", p!(&self.0))
    }
}

impl<T: Ordered> FromIterator<DepSpec<T>> for DepSet<T> {
    fn from_iter<I: IntoIterator<Item = DepSpec<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<T: Ordered> DepSet<T> {
    pub fn iter(&self) -> Iter<T> {
        self.into_iter()
    }

    pub fn iter_flatten(&self) -> IterFlatten<T> {
        self.into_iter_flatten()
    }
}

#[derive(Debug)]
pub struct Iter<'a, T: Ordered>(indexmap::set::Iter<'a, DepSpec<T>>);

impl<'a, T: Ordered> IntoIterator for &'a DepSet<T> {
    type Item = &'a DepSpec<T>;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.0.iter())
    }
}

impl<'a, T: Ordered> Iterator for Iter<'a, T> {
    type Item = &'a DepSpec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T: Ordered> Flatten for &'a DepSet<T> {
    type Item = &'a T;
    type IntoIterFlatten = IterFlatten<'a, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IterFlatten(self.0.iter().collect())
    }
}

impl<'a, T: Ordered> Recursive for &'a DepSet<T> {
    type Item = &'a DepSpec<T>;
    type IntoIterRecursive = IterRecursive<'a, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IterRecursive(self.0.iter().collect())
    }
}

#[derive(Debug)]
pub struct IterFlatten<'a, T: Ordered>(VecDeque<&'a DepSpec<T>>);

impl<'a, T: fmt::Debug + Ordered> Iterator for IterFlatten<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
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
pub struct IntoIter<T: Ordered>(indexmap::set::IntoIter<DepSpec<T>>);

impl<T: Ordered> IntoIterator for DepSet<T> {
    type Item = DepSpec<T>;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter())
    }
}

impl<T: Ordered> Iterator for IntoIter<T> {
    type Item = DepSpec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<T: Ordered> Flatten for DepSet<T> {
    type Item = T;
    type IntoIterFlatten = IntoIterFlatten<T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IntoIterFlatten(self.0.into_iter().collect())
    }
}

impl<T: Ordered> Recursive for DepSet<T> {
    type Item = DepSpec<T>;
    type IntoIterRecursive = IntoIterRecursive<T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IntoIterRecursive(self.0.into_iter().collect())
    }
}

#[derive(Debug)]
pub struct IntoIterFlatten<T: Ordered>(VecDeque<DepSpec<T>>);

impl<T: fmt::Debug + Ordered> Iterator for IntoIterFlatten<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
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
pub struct IterRecursive<'a, T: Ordered>(VecDeque<&'a DepSpec<T>>);

impl<'a, T: fmt::Debug + Ordered> Iterator for IterRecursive<'a, T> {
    type Item = &'a DepSpec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
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
pub struct IntoIterRecursive<T: Ordered>(VecDeque<DepSpec<T>>);

impl<T: fmt::Debug + Ordered> Iterator for IntoIterRecursive<T> {
    type Item = DepSpec<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
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

impl Restriction<&DepSpec<Dep>> for BaseRestrict {
    fn matches(&self, val: &DepSpec<Dep>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DepSet<Dep>> for BaseRestrict {
    fn matches(&self, val: &DepSet<Dep>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}
