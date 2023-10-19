use std::fmt;
use std::hash::Hash;

use indexmap::IndexSet;
use itertools::Itertools;

use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::types::{Deque, Ordered, OrderedSet, SortedSet};
use crate::Error;

use super::Dep;

pub trait UseFlag:
    fmt::Debug + fmt::Display + PartialEq + Eq + PartialOrd + Ord + Clone + AsRef<str> + Hash + ToString
{
}
impl UseFlag for String {}
impl UseFlag for &String {}

/// Evaluation support for dependency objects.
pub trait Evaluate<'a> {
    type Evaluated;
    fn evaluate(self, options: &'a IndexSet<String>) -> Self::Evaluated;
}

/// Evaluated iterator support for dependency objects.
pub trait EvaluateIter<'a> {
    type Item;
    type IntoIterEvaluate: Iterator<Item = Self::Item>;
    fn into_iter_evaluate(self, options: &'a IndexSet<String>) -> Self::IntoIterEvaluate;
}

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

/// Convert a borrowed type into an owned type.
pub trait IntoOwned {
    type Owned;
    fn into_owned(self) -> Self::Owned;
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
pub enum DepSpec<S: UseFlag, T: Ordered> {
    /// Enabled dependency.
    Enabled(T),
    /// Disabled dependency.
    Disabled(T), // REQUIRED_USE only
    /// All of a given dependency set.
    AllOf(SortedSet<Box<DepSpec<S, T>>>),
    /// Any of a given dependency set.
    AnyOf(OrderedSet<Box<DepSpec<S, T>>>),
    /// Exactly one of a given dependency set (REQUIRED_USE only).
    ExactlyOneOf(OrderedSet<Box<DepSpec<S, T>>>),
    /// At most of a given dependency set (REQUIRED_USE only).
    AtMostOneOf(OrderedSet<Box<DepSpec<S, T>>>),
    /// Conditionally enabled dependency.
    UseEnabled(S, SortedSet<Box<DepSpec<S, T>>>),
    /// Conditionally disabled dependency.
    UseDisabled(S, SortedSet<Box<DepSpec<S, T>>>),
}

macro_rules! box_ref {
    ($vals:expr) => {
        $vals
            .into_iter()
            .map(|b| Box::new(b.as_ref().as_ref()))
            .collect()
    };
}

impl<'a, T: Ordered> DepSpec<String, T> {
    pub fn as_ref(&'a self) -> DepSpec<&'a String, &'a T> {
        use DepSpec::*;
        match self {
            Enabled(ref val) => Enabled(val),
            Disabled(ref val) => Disabled(val),
            AllOf(ref vals) => AllOf(box_ref!(vals)),
            AnyOf(ref vals) => AnyOf(box_ref!(vals)),
            ExactlyOneOf(ref vals) => ExactlyOneOf(box_ref!(vals)),
            AtMostOneOf(ref vals) => AtMostOneOf(box_ref!(vals)),
            UseEnabled(u, ref vals) => UseEnabled(u, box_ref!(vals)),
            UseDisabled(u, ref vals) => UseDisabled(u, box_ref!(vals)),
        }
    }
}

macro_rules! box_owned {
    ($vals:expr) => {
        $vals
            .into_iter()
            .map(|b| Box::new(b.into_owned()))
            .collect()
    };
}

impl<T: Ordered> IntoOwned for DepSpec<&String, &T> {
    type Owned = DepSpec<String, T>;

    fn into_owned(self) -> Self::Owned {
        use DepSpec::*;
        match self {
            Enabled(val) => Enabled(val.clone()),
            Disabled(val) => Disabled(val.clone()),
            AllOf(vals) => AllOf(box_owned!(vals)),
            AnyOf(vals) => AnyOf(box_owned!(vals)),
            ExactlyOneOf(vals) => ExactlyOneOf(box_owned!(vals)),
            AtMostOneOf(vals) => AtMostOneOf(box_owned!(vals)),
            UseEnabled(u, vals) => UseEnabled(u.clone(), box_owned!(vals)),
            UseDisabled(u, vals) => UseDisabled(u.clone(), box_owned!(vals)),
        }
    }
}

impl<S: UseFlag, T: Ordered> DepSpec<S, T> {
    pub fn is_empty(&self) -> bool {
        use DepSpec::*;
        match self {
            Enabled(_) | Disabled(_) => false,
            AllOf(vals) => vals.iter().all(|d| d.is_empty()),
            AnyOf(vals) => vals.iter().all(|d| d.is_empty()),
            ExactlyOneOf(vals) => vals.iter().all(|d| d.is_empty()),
            AtMostOneOf(vals) => vals.iter().all(|d| d.is_empty()),
            UseEnabled(_, vals) => vals.iter().all(|d| d.is_empty()),
            UseDisabled(_, vals) => vals.iter().all(|d| d.is_empty()),
        }
    }

    pub fn iter_flatten(&self) -> IterFlatten<S, T> {
        self.into_iter_flatten()
    }

    pub fn iter_recursive(&self) -> IterRecursive<S, T> {
        self.into_iter_recursive()
    }
}

impl<'a, S: UseFlag, T: Ordered> EvaluateIter<'a> for &'a DepSpec<S, T> {
    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluate = IterEvaluate<'a, S, T>;

    fn into_iter_evaluate(self, options: &'a IndexSet<String>) -> Self::IntoIterEvaluate {
        IterEvaluate {
            q: [self].into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateIter<'a> for DepSpec<&'a String, &'a T> {
    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluate = IntoIterEvaluate<'a, T>;

    fn into_iter_evaluate(self, options: &'a IndexSet<String>) -> Self::IntoIterEvaluate {
        IntoIterEvaluate {
            q: [self].into_iter().collect(),
            options,
        }
    }
}

impl<'a, S: UseFlag, T: Ordered> Flatten for &'a DepSpec<S, T> {
    type Item = &'a T;
    type IntoIterFlatten = IterFlatten<'a, S, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IterFlatten([self].into_iter().collect())
    }
}

impl<'a, S: UseFlag, T: Ordered> Recursive for &'a DepSpec<S, T> {
    type Item = &'a DepSpec<S, T>;
    type IntoIterRecursive = IterRecursive<'a, S, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IterRecursive([self].into_iter().collect())
    }
}

impl<S: UseFlag, T: Ordered> Flatten for DepSpec<S, T> {
    type Item = T;
    type IntoIterFlatten = IntoIterFlatten<S, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IntoIterFlatten([self].into_iter().collect())
    }
}

impl<S: UseFlag, T: Ordered> Recursive for DepSpec<S, T> {
    type Item = DepSpec<S, T>;
    type IntoIterRecursive = IntoIterRecursive<S, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IntoIterRecursive([self].into_iter().collect())
    }
}

impl<S: UseFlag, T: fmt::Display + Ordered> fmt::Display for DepSpec<S, T> {
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
pub struct DepSet<S: UseFlag, T: Ordered>(SortedSet<DepSpec<S, T>>);

impl<S: UseFlag, T: Ordered> Default for DepSet<S, T> {
    fn default() -> Self {
        Self(SortedSet::new())
    }
}

impl<S: UseFlag, T: fmt::Display + Ordered> fmt::Display for DepSet<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", p!(&self.0))
    }
}

impl<S: UseFlag, T: Ordered> FromIterator<DepSpec<S, T>> for DepSet<S, T> {
    fn from_iter<I: IntoIterator<Item = DepSpec<S, T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, T: Ordered + 'a> FromIterator<&'a DepSpec<String, T>> for DepSet<&'a String, &'a T> {
    fn from_iter<I: IntoIterator<Item = &'a DepSpec<String, T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().map(|d| d.as_ref()).collect())
    }
}

impl<S: UseFlag, T: Ordered> DepSet<S, T> {
    pub fn is_empty(&self) -> bool {
        self.into_iter().all(|d| d.is_empty())
    }

    pub fn iter(&self) -> Iter<S, T> {
        self.into_iter()
    }

    pub fn iter_flatten(&self) -> IterFlatten<S, T> {
        self.into_iter_flatten()
    }

    pub fn iter_recursive(&self) -> IterRecursive<S, T> {
        self.into_iter_recursive()
    }
}

impl<'a, T: Ordered> Evaluate<'a> for &'a DepSet<String, T> {
    type Evaluated = DepSet<&'a String, &'a T>;

    fn evaluate(self, options: &'a IndexSet<String>) -> Self::Evaluated {
        DepSet(self.into_iter_evaluate(options).collect())
    }
}

// TODO: combine with &DepSet<String, T> impl using a macro
impl<'a, T: Ordered> Evaluate<'a> for DepSet<&'a String, &'a T> {
    type Evaluated = DepSet<&'a String, &'a T>;

    fn evaluate(self, options: &'a IndexSet<String>) -> Self::Evaluated {
        DepSet(self.into_iter_evaluate(options).collect())
    }
}

impl<T: Ordered> IntoOwned for DepSet<&String, &T> {
    type Owned = DepSet<String, T>;

    fn into_owned(self) -> Self::Owned {
        DepSet(self.into_iter().map(|d| d.into_owned()).collect())
    }
}

#[derive(Debug)]
pub struct Iter<'a, S: UseFlag, T: Ordered>(indexmap::set::Iter<'a, DepSpec<S, T>>);

impl<'a, S: UseFlag, T: Ordered> IntoIterator for &'a DepSet<S, T> {
    type Item = &'a DepSpec<S, T>;
    type IntoIter = Iter<'a, S, T>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.0.iter())
    }
}

impl<'a, S: UseFlag, T: Ordered> Iterator for Iter<'a, S, T> {
    type Item = &'a DepSpec<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, S: UseFlag, T: Ordered> EvaluateIter<'a> for &'a DepSet<S, T> {
    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluate = IterEvaluate<'a, S, T>;

    fn into_iter_evaluate(self, options: &'a IndexSet<String>) -> Self::IntoIterEvaluate {
        IterEvaluate {
            q: self.0.iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateIter<'a> for DepSet<&'a String, &'a T> {
    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluate = IntoIterEvaluate<'a, T>;

    fn into_iter_evaluate(self, options: &'a IndexSet<String>) -> Self::IntoIterEvaluate {
        IntoIterEvaluate {
            q: self.0.into_iter().collect(),
            options,
        }
    }
}

impl<'a, S: UseFlag, T: Ordered> Flatten for &'a DepSet<S, T> {
    type Item = &'a T;
    type IntoIterFlatten = IterFlatten<'a, S, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IterFlatten(self.0.iter().collect())
    }
}

impl<'a, S: UseFlag, T: Ordered> Recursive for &'a DepSet<S, T> {
    type Item = &'a DepSpec<S, T>;
    type IntoIterRecursive = IterRecursive<'a, S, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IterRecursive(self.0.iter().collect())
    }
}

macro_rules! iter_eval {
    ($variant:expr, $vals:expr, $options:expr) => {{
        let dep = $variant(
            $vals
                .into_iter()
                .flat_map(|d| d.into_iter_evaluate($options))
                .map(|d| Box::new(d))
                .collect(),
        );

        if !dep.is_empty() {
            return Some(dep);
        }
    }};
}

#[derive(Debug)]
pub struct IterEvaluate<'a, S: UseFlag, T: Ordered> {
    q: Deque<&'a DepSpec<S, T>>,
    options: &'a IndexSet<String>,
}

impl<'a, S: UseFlag, T: fmt::Debug + Ordered> Iterator for IterEvaluate<'a, S, T> {
    type Item = DepSpec<&'a String, &'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval!(AllOf, vals, self.options),
                AnyOf(vals) => iter_eval!(AnyOf, vals, self.options),
                ExactlyOneOf(vals) => iter_eval!(ExactlyOneOf, vals, self.options),
                AtMostOneOf(vals) => iter_eval!(AtMostOneOf, vals, self.options),
                UseEnabled(flag, vals) => {
                    if self.options.contains(flag.as_ref()) {
                        self.q.extend_left(vals.into_iter().map(AsRef::as_ref));
                    }
                }
                UseDisabled(flag, vals) => {
                    if !self.options.contains(flag.as_ref()) {
                        self.q.extend_left(vals.into_iter().map(AsRef::as_ref));
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IterFlatten<'a, S: UseFlag, T: Ordered>(Deque<&'a DepSpec<S, T>>);

impl<'a, S: UseFlag, T: fmt::Debug + Ordered> Iterator for IterFlatten<'a, S, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                UseEnabled(_, vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                UseDisabled(_, vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIter<S: UseFlag, T: Ordered>(indexmap::set::IntoIter<DepSpec<S, T>>);

impl<S: UseFlag, T: Ordered> IntoIterator for DepSet<S, T> {
    type Item = DepSpec<S, T>;
    type IntoIter = IntoIter<S, T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter())
    }
}

impl<S: UseFlag, T: Ordered> Iterator for IntoIter<S, T> {
    type Item = DepSpec<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<S: UseFlag, T: Ordered> Flatten for DepSet<S, T> {
    type Item = T;
    type IntoIterFlatten = IntoIterFlatten<S, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IntoIterFlatten(self.0.into_iter().collect())
    }
}

impl<S: UseFlag, T: Ordered> Recursive for DepSet<S, T> {
    type Item = DepSpec<S, T>;
    type IntoIterRecursive = IntoIterRecursive<S, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IntoIterRecursive(self.0.into_iter().collect())
    }
}

#[derive(Debug)]
pub struct IntoIterEvaluate<'a, T: Ordered> {
    q: Deque<DepSpec<&'a String, &'a T>>,
    options: &'a IndexSet<String>,
}

impl<'a, T: fmt::Debug + Ordered> Iterator for IntoIterEvaluate<'a, T> {
    type Item = DepSpec<&'a String, &'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval!(AllOf, vals, self.options),
                AnyOf(vals) => iter_eval!(AnyOf, vals, self.options),
                ExactlyOneOf(vals) => iter_eval!(ExactlyOneOf, vals, self.options),
                AtMostOneOf(vals) => iter_eval!(AtMostOneOf, vals, self.options),
                UseEnabled(flag, vals) => {
                    if self.options.contains(flag) {
                        self.q.extend_left(vals.into_iter().map(|x| *x));
                    }
                }
                UseDisabled(flag, vals) => {
                    if !self.options.contains(flag) {
                        self.q.extend_left(vals.into_iter().map(|x| *x));
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterFlatten<S: UseFlag, T: Ordered>(Deque<DepSpec<S, T>>);

impl<S: UseFlag, T: fmt::Debug + Ordered> Iterator for IntoIterFlatten<S, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AnyOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AtMostOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                UseEnabled(_, vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                UseDisabled(_, vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IterRecursive<'a, S: UseFlag, T: Ordered>(Deque<&'a DepSpec<S, T>>);

impl<'a, S: UseFlag, T: fmt::Debug + Ordered> Iterator for IterRecursive<'a, S, T> {
    type Item = &'a DepSpec<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        let val = self.0.pop_front();
        if let Some(dep) = val {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                UseEnabled(_, vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                UseDisabled(_, vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
            }
        }

        val
    }
}

#[derive(Debug)]
pub struct IntoIterRecursive<S: UseFlag, T: Ordered>(Deque<DepSpec<S, T>>);

impl<S: UseFlag, T: fmt::Debug + Ordered> Iterator for IntoIterRecursive<S, T> {
    type Item = DepSpec<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        let val = self.0.pop_front();
        if let Some(dep) = &val {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
                AnyOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
                ExactlyOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
                AtMostOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
                UseEnabled(_, vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
                UseDisabled(_, vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
            }
        }

        val
    }
}

impl Restriction<&DepSpec<String, Dep>> for BaseRestrict {
    fn matches(&self, val: &DepSpec<String, Dep>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DepSet<String, Dep>> for BaseRestrict {
    fn matches(&self, val: &DepSet<String, Dep>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}
