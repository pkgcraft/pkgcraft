use std::borrow::Borrow;
use std::fmt;
use std::hash::Hash;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Sub, SubAssign};

use indexmap::IndexSet;
use itertools::Itertools;

use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::Contains;
use crate::types::{Deque, Ordered, OrderedSet, SortedSet};
use crate::Error;

use super::Dep;

pub trait UseFlag:
    fmt::Debug + fmt::Display + PartialEq + Eq + PartialOrd + Ord + Clone + Hash
{
}
impl<T> UseFlag for T where
    T: fmt::Debug + fmt::Display + PartialEq + Eq + PartialOrd + Ord + Clone + Hash
{
}

pub trait Enabled: Hash + Borrow<str> + PartialEq + Eq {}
impl<T> Enabled for T where T: Hash + Borrow<str> + PartialEq + Eq {}

/// Evaluation support for dependency objects.
pub trait Evaluate<'a, S: Enabled + 'a> {
    type Evaluated;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated;

    type Item;
    type IntoIterEvaluate: Iterator<Item = Self::Item>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate;
}

/// Forced evaluation support for dependency objects.
pub trait EvaluateForce {
    type Evaluated;
    fn evaluate_force(self, force: bool) -> Self::Evaluated;

    type Item;
    type IntoIterEvaluateForce: Iterator<Item = Self::Item>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce;
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

/// Conditional iterator support for dependency objects.
pub trait Conditionals {
    type Item;
    type IntoIterConditionals: Iterator<Item = Self::Item>;
    fn into_iter_conditionals(self) -> Self::IntoIterConditionals;
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
    /// Disabled dependency (REQUIRED_USE only).
    Disabled(T),
    /// All of a given dependency set.
    AllOf(SortedSet<Box<DepSpec<S, T>>>),
    /// Any of a given dependency set.
    AnyOf(OrderedSet<Box<DepSpec<S, T>>>),
    /// Exactly one of a given dependency set (REQUIRED_USE only).
    ExactlyOneOf(OrderedSet<Box<DepSpec<S, T>>>),
    /// At most one of a given dependency set (REQUIRED_USE only).
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
    /// Converts from `&DepSpec<String, T>` to `DepSpec<&String, &T>`.
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
            AllOf(vals) => vals.is_empty(),
            AnyOf(vals) => vals.is_empty(),
            ExactlyOneOf(vals) => vals.is_empty(),
            AtMostOneOf(vals) => vals.is_empty(),
            UseEnabled(_, vals) => vals.is_empty(),
            UseDisabled(_, vals) => vals.is_empty(),
        }
    }

    /// Return the number of `DepSpec` objects a `DepSpec` contains.
    pub fn len(&self) -> usize {
        use DepSpec::*;
        match self {
            Enabled(_) => 1,
            Disabled(_) => 1,
            AllOf(vals) => vals.len(),
            AnyOf(vals) => vals.len(),
            ExactlyOneOf(vals) => vals.len(),
            AtMostOneOf(vals) => vals.len(),
            UseEnabled(_, vals) => vals.len(),
            UseDisabled(_, vals) => vals.len(),
        }
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

    pub fn iter_conditionals(&self) -> IterConditionals<S, T> {
        self.into_iter_conditionals()
    }
}

impl<S: UseFlag, T: Ordered> Contains<&Self> for DepSpec<S, T> {
    fn contains(&self, dep: &Self) -> bool {
        use DepSpec::*;
        match self {
            Enabled(_) | Disabled(_) => false,
            AllOf(vals) => vals.contains(dep),
            AnyOf(vals) => vals.contains(dep),
            ExactlyOneOf(vals) => vals.contains(dep),
            AtMostOneOf(vals) => vals.contains(dep),
            UseEnabled(_, vals) => vals.contains(dep),
            UseDisabled(_, vals) => vals.contains(dep),
        }
    }
}

impl<'a, S: UseFlag, T: Ordered> IntoIterator for &'a DepSpec<S, T> {
    type Item = &'a DepSpec<S, T>;
    type IntoIter = Iter<'a, S, T>;

    fn into_iter(self) -> Self::IntoIter {
        use DepSpec::*;
        match self {
            Enabled(_) | Disabled(_) => [].into_iter().collect(),
            AllOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            AnyOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            ExactlyOneOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            AtMostOneOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            UseEnabled(_, vals) => vals.iter().map(AsRef::as_ref).collect(),
            UseDisabled(_, vals) => vals.iter().map(AsRef::as_ref).collect(),
        }
    }
}

impl<'a, S: Enabled + 'a, T: Ordered> Evaluate<'a, S> for &'a DepSpec<String, T> {
    type Evaluated = SortedSet<DepSpec<&'a String, &'a T>>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluate = IterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IterEvaluate {
            q: [self].into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for &'a DepSpec<String, T> {
    type Evaluated = SortedSet<DepSpec<&'a String, &'a T>>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluateForce = IterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IterEvaluateForce {
            q: [self].into_iter().collect(),
            force,
        }
    }
}

impl<'a, S: Enabled + 'a, T: Ordered> Evaluate<'a, S> for DepSpec<&'a String, &'a T> {
    type Evaluated = SortedSet<DepSpec<&'a String, &'a T>>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluate = IntoIterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IntoIterEvaluate {
            q: [self].into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for DepSpec<&'a String, &'a T> {
    type Evaluated = SortedSet<DepSpec<&'a String, &'a T>>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluateForce = IntoIterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IntoIterEvaluateForce {
            q: [self].into_iter().collect(),
            force,
        }
    }
}

impl<S: UseFlag, T: Ordered> IntoIterator for DepSpec<S, T> {
    type Item = DepSpec<S, T>;
    type IntoIter = IntoIter<S, T>;

    fn into_iter(self) -> Self::IntoIter {
        use DepSpec::*;
        match self {
            Enabled(_) | Disabled(_) => [].into_iter().collect(),
            AllOf(vals) => vals.into_iter().map(|x| *x).collect(),
            AnyOf(vals) => vals.into_iter().map(|x| *x).collect(),
            ExactlyOneOf(vals) => vals.into_iter().map(|x| *x).collect(),
            AtMostOneOf(vals) => vals.into_iter().map(|x| *x).collect(),
            UseEnabled(_, vals) => vals.into_iter().map(|x| *x).collect(),
            UseDisabled(_, vals) => vals.into_iter().map(|x| *x).collect(),
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

impl<'a, S: UseFlag, T: Ordered> Conditionals for &'a DepSpec<S, T> {
    type Item = &'a S;
    type IntoIterConditionals = IterConditionals<'a, S, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IterConditionals([self].into_iter().collect())
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

impl<S: UseFlag, T: Ordered> Conditionals for DepSpec<S, T> {
    type Item = S;
    type IntoIterConditionals = IntoIterConditionals<S, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IntoIterConditionals([self].into_iter().collect())
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

impl<S: UseFlag, T: Ordered> DepSet<S, T> {
    /// Construct a new, empty `DepSet<S, T>`.
    pub fn new() -> Self {
        Self(SortedSet::new())
    }

    /// Return the number of `DepSpec` objects a `DepSet` contains.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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

    pub fn iter_conditionals(&self) -> IterConditionals<S, T> {
        self.into_iter_conditionals()
    }
}

impl<S: UseFlag, T: Ordered> Default for DepSet<S, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: UseFlag, T: fmt::Display + Ordered> fmt::Display for DepSet<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", p!(self))
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

impl<S: UseFlag, T: Ordered> BitAnd<&Self> for DepSet<S, T> {
    type Output = Self;

    fn bitand(mut self, other: &Self) -> Self::Output {
        self &= other;
        self
    }
}

impl<S: UseFlag, T: Ordered> BitAndAssign<&Self> for DepSet<S, T> {
    fn bitand_assign(&mut self, other: &Self) {
        self.0 &= &other.0;
    }
}

impl<S: UseFlag, T: Ordered> BitOr<&Self> for DepSet<S, T> {
    type Output = Self;

    fn bitor(mut self, other: &Self) -> Self::Output {
        self |= other;
        self
    }
}

impl<S: UseFlag, T: Ordered> BitOrAssign<&Self> for DepSet<S, T> {
    fn bitor_assign(&mut self, other: &Self) {
        self.0 |= &other.0;
    }
}

impl<S: UseFlag, T: Ordered> BitXor<&Self> for DepSet<S, T> {
    type Output = Self;

    fn bitxor(mut self, other: &Self) -> Self::Output {
        self ^= other;
        self
    }
}

impl<S: UseFlag, T: Ordered> BitXorAssign<&Self> for DepSet<S, T> {
    fn bitxor_assign(&mut self, other: &Self) {
        self.0 ^= &other.0;
    }
}

impl<S: UseFlag, T: Ordered> Sub<&Self> for DepSet<S, T> {
    type Output = Self;

    fn sub(mut self, other: &Self) -> Self::Output {
        self -= other;
        self
    }
}

impl<S: UseFlag, T: Ordered> SubAssign<&Self> for DepSet<S, T> {
    fn sub_assign(&mut self, other: &Self) {
        self.0 -= &other.0;
    }
}

impl<S: UseFlag, T: Ordered> Contains<&DepSpec<S, T>> for DepSet<S, T> {
    fn contains(&self, dep: &DepSpec<S, T>) -> bool {
        self.0.contains(dep)
    }
}

impl<'a, S: Enabled + 'a, T: Ordered> Evaluate<'a, S> for &'a DepSet<String, T> {
    type Evaluated = DepSet<&'a String, &'a T>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluate = IterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IterEvaluate {
            q: self.0.iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for &'a DepSet<String, T> {
    type Evaluated = DepSet<&'a String, &'a T>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluateForce = IterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IterEvaluateForce {
            q: self.0.iter().collect(),
            force,
        }
    }
}

impl<'a, S: Enabled + 'a, T: Ordered> Evaluate<'a, S> for DepSet<&'a String, &'a T> {
    type Evaluated = DepSet<&'a String, &'a T>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluate = IntoIterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IntoIterEvaluate {
            q: self.0.into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for DepSet<&'a String, &'a T> {
    type Evaluated = DepSet<&'a String, &'a T>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = DepSpec<&'a String, &'a T>;
    type IntoIterEvaluateForce = IntoIterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IntoIterEvaluateForce {
            q: self.0.into_iter().collect(),
            force,
        }
    }
}

impl<T: Ordered> IntoOwned for DepSet<&String, &T> {
    type Owned = DepSet<String, T>;

    fn into_owned(self) -> Self::Owned {
        self.into_iter().map(|d| d.into_owned()).collect()
    }
}

#[derive(Debug)]
pub struct Iter<'a, S: UseFlag, T: Ordered>(Deque<&'a DepSpec<S, T>>);

impl<'a, S: UseFlag, T: Ordered> FromIterator<&'a DepSpec<S, T>> for Iter<'a, S, T> {
    fn from_iter<I: IntoIterator<Item = &'a DepSpec<S, T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, S: UseFlag, T: Ordered> Iterator for Iter<'a, S, T> {
    type Item = &'a DepSpec<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front()
    }
}

impl<'a, S: UseFlag, T: Ordered> IntoIterator for &'a DepSet<S, T> {
    type Item = &'a DepSpec<S, T>;
    type IntoIter = Iter<'a, S, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().collect()
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

impl<'a, S: UseFlag, T: Ordered> Conditionals for &'a DepSet<S, T> {
    type Item = &'a S;
    type IntoIterConditionals = IterConditionals<'a, S, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IterConditionals(self.0.iter().collect())
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
pub struct IterEvaluate<'a, S: Enabled, T: Ordered> {
    q: Deque<&'a DepSpec<String, T>>,
    options: &'a IndexSet<S>,
}

impl<'a, S: Enabled, T: fmt::Debug + Ordered> Iterator for IterEvaluate<'a, S, T> {
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
                    if self.options.contains(flag.as_str()) {
                        self.q.extend_left(vals.into_iter().map(AsRef::as_ref));
                    }
                }
                UseDisabled(flag, vals) => {
                    if !self.options.contains(flag.as_str()) {
                        self.q.extend_left(vals.into_iter().map(AsRef::as_ref));
                    }
                }
            }
        }
        None
    }
}

macro_rules! iter_eval_force {
    ($variant:expr, $vals:expr, $force:expr) => {{
        let dep = $variant(
            $vals
                .into_iter()
                .flat_map(|d| d.into_iter_evaluate_force($force))
                .map(|d| Box::new(d))
                .collect(),
        );

        if !dep.is_empty() {
            return Some(dep);
        }
    }};
}

#[derive(Debug)]
pub struct IterEvaluateForce<'a, T: Ordered> {
    q: Deque<&'a DepSpec<String, T>>,
    force: bool,
}

impl<'a, T: fmt::Debug + Ordered> Iterator for IterEvaluateForce<'a, T> {
    type Item = DepSpec<&'a String, &'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval_force!(AllOf, vals, self.force),
                AnyOf(vals) => iter_eval_force!(AnyOf, vals, self.force),
                ExactlyOneOf(vals) => iter_eval_force!(ExactlyOneOf, vals, self.force),
                AtMostOneOf(vals) => iter_eval_force!(AtMostOneOf, vals, self.force),
                UseEnabled(_, vals) | UseDisabled(_, vals) => {
                    if self.force {
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
pub struct IntoIter<S: UseFlag, T: Ordered>(Deque<DepSpec<S, T>>);

impl<S: UseFlag, T: Ordered> FromIterator<DepSpec<S, T>> for IntoIter<S, T> {
    fn from_iter<I: IntoIterator<Item = DepSpec<S, T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<S: UseFlag, T: Ordered> Iterator for IntoIter<S, T> {
    type Item = DepSpec<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front()
    }
}

impl<S: UseFlag, T: Ordered> IntoIterator for DepSet<S, T> {
    type Item = DepSpec<S, T>;
    type IntoIter = IntoIter<S, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().collect()
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

impl<S: UseFlag, T: Ordered> Conditionals for DepSet<S, T> {
    type Item = S;
    type IntoIterConditionals = IntoIterConditionals<S, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IntoIterConditionals(self.0.into_iter().collect())
    }
}

#[derive(Debug)]
pub struct IntoIterEvaluate<'a, S: Enabled, T: Ordered> {
    q: Deque<DepSpec<&'a String, &'a T>>,
    options: &'a IndexSet<S>,
}

impl<'a, S: Enabled, T: fmt::Debug + Ordered> Iterator for IntoIterEvaluate<'a, S, T> {
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
                    if self.options.contains(flag.as_str()) {
                        self.q.extend_left(vals.into_iter().map(|x| *x));
                    }
                }
                UseDisabled(flag, vals) => {
                    if !self.options.contains(flag.as_str()) {
                        self.q.extend_left(vals.into_iter().map(|x| *x));
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterEvaluateForce<'a, T: Ordered> {
    q: Deque<DepSpec<&'a String, &'a T>>,
    force: bool,
}

impl<'a, T: fmt::Debug + Ordered> Iterator for IntoIterEvaluateForce<'a, T> {
    type Item = DepSpec<&'a String, &'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval_force!(AllOf, vals, self.force),
                AnyOf(vals) => iter_eval_force!(AnyOf, vals, self.force),
                ExactlyOneOf(vals) => iter_eval_force!(ExactlyOneOf, vals, self.force),
                AtMostOneOf(vals) => iter_eval_force!(AtMostOneOf, vals, self.force),
                UseEnabled(_, vals) | UseDisabled(_, vals) => {
                    if self.force {
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

#[derive(Debug)]
pub struct IterConditionals<'a, S: UseFlag, T: Ordered>(Deque<&'a DepSpec<S, T>>);

impl<'a, S: UseFlag, T: fmt::Debug + Ordered> Iterator for IterConditionals<'a, S, T> {
    type Item = &'a S;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                UseEnabled(flag, vals) | UseDisabled(flag, vals) => {
                    self.0.extend_left(vals.iter().map(AsRef::as_ref));
                    return Some(flag);
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterConditionals<S: UseFlag, T: Ordered>(Deque<DepSpec<S, T>>);

impl<S: UseFlag, T: fmt::Debug + Ordered> Iterator for IntoIterConditionals<S, T> {
    type Item = S;

    fn next(&mut self) -> Option<Self::Item> {
        use DepSpec::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AnyOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AtMostOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                UseEnabled(flag, vals) | UseDisabled(flag, vals) => {
                    self.0.extend_left(vals.into_iter().map(|x| *x));
                    return Some(flag);
                }
            }
        }
        None
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
