use std::borrow::Borrow;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Sub, SubAssign};
use std::str::FromStr;

use indexmap::IndexSet;
use itertools::Itertools;

use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::{Contains, IntoOwned, ToRef};
use crate::types::{Deque, Ordered, OrderedSet, SortedSet};
use crate::Error;

pub mod cpv;
pub mod parse;
pub mod pkg;
pub mod uri;
pub mod use_dep;
pub mod version;

pub use cpv::{Cpv, CpvOrDep};
pub use pkg::{Blocker, Dep, DepField, Slot, SlotDep, SlotOperator};
pub use uri::Uri;
pub use use_dep::{UseDep, UseDepDefault, UseDepKind};
pub use version::{Operator, Revision, Version};

pub trait Stringable:
    Debug
    + Display
    + Default
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + Clone
    + Hash
    + Borrow<str>
    + AsRef<str>
{
}

impl<T> Stringable for T where
    T: Debug
        + Display
        + Default
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Clone
        + Hash
        + Borrow<str>
        + AsRef<str>
{
}

/// Evaluation support for dependency objects.
pub trait Evaluate<'a, S: Stringable + 'a> {
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

macro_rules! p {
    ($x:expr) => {
        $x.into_iter().map(|x| x.to_string()).join(" ")
    };
}

/// Dependency specification variants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dependency<S: Stringable, T: Ordered> {
    /// Enabled dependency.
    Enabled(T),
    /// Disabled dependency (REQUIRED_USE only).
    Disabled(T),
    /// All of a given dependency set.
    AllOf(SortedSet<Box<Dependency<S, T>>>),
    /// Any of a given dependency set.
    AnyOf(OrderedSet<Box<Dependency<S, T>>>),
    /// Exactly one of a given dependency set (REQUIRED_USE only).
    ExactlyOneOf(OrderedSet<Box<Dependency<S, T>>>),
    /// At most one of a given dependency set (REQUIRED_USE only).
    AtMostOneOf(OrderedSet<Box<Dependency<S, T>>>),
    /// USE conditional dependency.
    UseConditional(UseDep<S>, SortedSet<Box<Dependency<S, T>>>),
}

macro_rules! box_owned {
    ($vals:expr) => {
        $vals
            .into_iter()
            .map(|b| Box::new(b.into_owned()))
            .collect()
    };
}

/// Recursively sort a set with sortable elements into an iterator.
// TODO: replace with in-place mutation when IndexSet supports IndexMut and/or iter_mut()
macro_rules! sort_set {
    ($vals:expr) => {
        itertools::sorted($vals.clone().into_iter().map(|mut d| {
            d.sort();
            d
        }))
    };
}

impl<T: Ordered> IntoOwned for Dependency<&str, &T> {
    type Owned = Dependency<String, T>;

    fn into_owned(self) -> Self::Owned {
        use Dependency::*;
        match self {
            Enabled(val) => Enabled(val.clone()),
            Disabled(val) => Disabled(val.clone()),
            AllOf(vals) => AllOf(box_owned!(vals)),
            AnyOf(vals) => AnyOf(box_owned!(vals)),
            ExactlyOneOf(vals) => ExactlyOneOf(box_owned!(vals)),
            AtMostOneOf(vals) => AtMostOneOf(box_owned!(vals)),
            UseConditional(u, vals) => UseConditional(u.into_owned(), box_owned!(vals)),
        }
    }
}

macro_rules! box_ref {
    ($vals:expr) => {
        $vals
            .into_iter()
            .map(|b| Box::new(b.as_ref().to_ref()))
            .collect()
    };
}

impl<'a, T: Ordered + 'a> ToRef<'a> for Dependency<String, T> {
    type Ref = Dependency<&'a str, &'a T>;

    fn to_ref(&'a self) -> Self::Ref {
        use Dependency::*;
        match self {
            Enabled(ref val) => Enabled(val),
            Disabled(ref val) => Disabled(val),
            AllOf(ref vals) => AllOf(box_ref!(vals)),
            AnyOf(ref vals) => AnyOf(box_ref!(vals)),
            ExactlyOneOf(ref vals) => ExactlyOneOf(box_ref!(vals)),
            AtMostOneOf(ref vals) => AtMostOneOf(box_ref!(vals)),
            UseConditional(u, ref vals) => UseConditional(u.to_ref(), box_ref!(vals)),
        }
    }
}

impl<S: Stringable, T: Ordered> Dependency<S, T> {
    pub fn is_empty(&self) -> bool {
        use Dependency::*;
        match self {
            Enabled(_) | Disabled(_) => false,
            AllOf(vals) => vals.is_empty(),
            AnyOf(vals) => vals.is_empty(),
            ExactlyOneOf(vals) => vals.is_empty(),
            AtMostOneOf(vals) => vals.is_empty(),
            UseConditional(_, vals) => vals.is_empty(),
        }
    }

    /// Return the number of `Dependency` objects a `Dependency` contains.
    pub fn len(&self) -> usize {
        use Dependency::*;
        match self {
            Enabled(_) => 1,
            Disabled(_) => 1,
            AllOf(vals) => vals.len(),
            AnyOf(vals) => vals.len(),
            ExactlyOneOf(vals) => vals.len(),
            AtMostOneOf(vals) => vals.len(),
            UseConditional(_, vals) => vals.len(),
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

    /// Recursively sort a `Dependency`.
    pub fn sort(&mut self) {
        use Dependency::*;
        match self {
            AllOf(vals) => *vals = sort_set!(vals).collect(),
            UseConditional(_, vals) => *vals = sort_set!(vals).collect(),
            _ => (),
        }
    }
}

impl FromStr for Dependency<String, Dep<String>> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let eapi = Default::default();
        parse::package_dependency(s, eapi)
    }
}

impl<S: Stringable, T: Ordered> Contains<&Self> for Dependency<S, T> {
    fn contains(&self, dep: &Self) -> bool {
        use Dependency::*;
        match self {
            Enabled(_) | Disabled(_) => false,
            AllOf(vals) => vals.contains(dep),
            AnyOf(vals) => vals.contains(dep),
            ExactlyOneOf(vals) => vals.contains(dep),
            AtMostOneOf(vals) => vals.contains(dep),
            UseConditional(_, vals) => vals.contains(dep),
        }
    }
}

impl<S: Stringable, T: Ordered> Contains<&T> for Dependency<S, T> {
    fn contains(&self, obj: &T) -> bool {
        self.iter_flatten().any(|x| x == obj)
    }
}

impl<'a, S: Stringable, T: Ordered> IntoIterator for &'a Dependency<S, T> {
    type Item = &'a Dependency<S, T>;
    type IntoIter = Iter<'a, S, T>;

    fn into_iter(self) -> Self::IntoIter {
        use Dependency::*;
        match self {
            Enabled(_) | Disabled(_) => [].into_iter().collect(),
            AllOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            AnyOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            ExactlyOneOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            AtMostOneOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            UseConditional(_, vals) => vals.iter().map(AsRef::as_ref).collect(),
        }
    }
}

impl<'a, S: Stringable + 'a, T: Ordered> Evaluate<'a, S> for &'a Dependency<String, T> {
    type Evaluated = SortedSet<Dependency<&'a str, &'a T>>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = Dependency<&'a str, &'a T>;
    type IntoIterEvaluate = IterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IterEvaluate {
            q: [self].into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for &'a Dependency<String, T> {
    type Evaluated = SortedSet<Dependency<&'a str, &'a T>>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = Dependency<&'a str, &'a T>;
    type IntoIterEvaluateForce = IterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IterEvaluateForce {
            q: [self].into_iter().collect(),
            force,
        }
    }
}

impl<'a, S: Stringable + 'a, T: Ordered> Evaluate<'a, S> for Dependency<&'a str, &'a T> {
    type Evaluated = SortedSet<Dependency<&'a str, &'a T>>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = Dependency<&'a str, &'a T>;
    type IntoIterEvaluate = IntoIterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IntoIterEvaluate {
            q: [self].into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for Dependency<&'a str, &'a T> {
    type Evaluated = SortedSet<Dependency<&'a str, &'a T>>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = Dependency<&'a str, &'a T>;
    type IntoIterEvaluateForce = IntoIterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IntoIterEvaluateForce {
            q: [self].into_iter().collect(),
            force,
        }
    }
}

impl<S: Stringable, T: Ordered> IntoIterator for Dependency<S, T> {
    type Item = Dependency<S, T>;
    type IntoIter = IntoIter<S, T>;

    fn into_iter(self) -> Self::IntoIter {
        use Dependency::*;
        match self {
            Enabled(_) | Disabled(_) => [].into_iter().collect(),
            AllOf(vals) => vals.into_iter().map(|x| *x).collect(),
            AnyOf(vals) => vals.into_iter().map(|x| *x).collect(),
            ExactlyOneOf(vals) => vals.into_iter().map(|x| *x).collect(),
            AtMostOneOf(vals) => vals.into_iter().map(|x| *x).collect(),
            UseConditional(_, vals) => vals.into_iter().map(|x| *x).collect(),
        }
    }
}

impl<'a, S: Stringable, T: Ordered> Flatten for &'a Dependency<S, T> {
    type Item = &'a T;
    type IntoIterFlatten = IterFlatten<'a, S, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IterFlatten([self].into_iter().collect())
    }
}

impl<'a, S: Stringable, T: Ordered> Recursive for &'a Dependency<S, T> {
    type Item = &'a Dependency<S, T>;
    type IntoIterRecursive = IterRecursive<'a, S, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IterRecursive([self].into_iter().collect())
    }
}

impl<'a, S: Stringable, T: Ordered> Conditionals for &'a Dependency<S, T> {
    type Item = &'a UseDep<S>;
    type IntoIterConditionals = IterConditionals<'a, S, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IterConditionals([self].into_iter().collect())
    }
}

impl<S: Stringable, T: Ordered> Flatten for Dependency<S, T> {
    type Item = T;
    type IntoIterFlatten = IntoIterFlatten<S, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IntoIterFlatten([self].into_iter().collect())
    }
}

impl<S: Stringable, T: Ordered> Recursive for Dependency<S, T> {
    type Item = Dependency<S, T>;
    type IntoIterRecursive = IntoIterRecursive<S, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IntoIterRecursive([self].into_iter().collect())
    }
}

impl<S: Stringable, T: Ordered> Conditionals for Dependency<S, T> {
    type Item = UseDep<S>;
    type IntoIterConditionals = IntoIterConditionals<S, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IntoIterConditionals([self].into_iter().collect())
    }
}

impl<S: Stringable, T: Display + Ordered> Display for Dependency<S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Dependency::*;
        match self {
            Enabled(val) => write!(f, "{val}"),
            Disabled(val) => write!(f, "!{val}"),
            AllOf(vals) => write!(f, "( {} )", p!(vals)),
            AnyOf(vals) => write!(f, "|| ( {} )", p!(vals)),
            ExactlyOneOf(vals) => write!(f, "^^ ( {} )", p!(vals)),
            AtMostOneOf(vals) => write!(f, "?? ( {} )", p!(vals)),
            UseConditional(u, vals) => write!(f, "{u} ( {} )", p!(vals)),
        }
    }
}

/// Set of dependency objects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DependencySet<S: Stringable, T: Ordered>(SortedSet<Dependency<S, T>>);

impl<S: Stringable, T: Ordered> DependencySet<S, T> {
    /// Construct a new, empty `DependencySet<S, T>`.
    pub fn new() -> Self {
        Self(SortedSet::new())
    }

    /// Return the `Dependency` for a given index.
    pub fn get_index(&self, index: usize) -> Option<&Dependency<S, T>> {
        self.0.get_index(index)
    }

    /// Insert a `Dependency` into the `DependencySet`.
    pub fn insert(&mut self, value: Dependency<S, T>) -> bool {
        self.0.insert(value)
    }

    /// Remove the last value.
    pub fn pop(&mut self) -> Option<Dependency<S, T>> {
        self.0.pop()
    }

    /// Recursively sort a `DependencySet`.
    pub fn sort(&mut self) {
        self.0 = sort_set!(self.0).collect();
    }

    /// Replace a `Dependency` with another `Dependency`, returning the replaced value.
    ///
    /// This removes the given element if its replacement value already exists by shifting all of
    /// the elements that follow it, preserving their relative order. **This perturbs the index of
    /// all of those elements!**
    pub fn shift_replace(
        &mut self,
        key: &Dependency<S, T>,
        value: Dependency<S, T>,
    ) -> Option<Dependency<S, T>> {
        self.0
            .get_index_of(key)
            .and_then(|i| self.shift_replace_index(i, value))
    }

    /// Replace a `Dependency` with another `Dependency`, returning the replaced value.
    ///
    /// This removes the given element if its replacement value already exists by swapping it with
    /// the last element of the set and popping it off. **This perturbs the position of what used
    /// to be the last element!**
    pub fn swap_replace(
        &mut self,
        key: &Dependency<S, T>,
        value: Dependency<S, T>,
    ) -> Option<Dependency<S, T>> {
        self.0
            .get_index_of(key)
            .and_then(|i| self.swap_replace_index(i, value))
    }

    /// Replace a `Dependency` for a given index in a `DependencySet`, returning the replaced value.
    ///
    /// This removes the element at the given index if its replacement value already exists by
    /// shifting all of the elements that follow it, preserving their relative order. **This
    /// perturbs the index of all of those elements!**
    pub fn shift_replace_index(
        &mut self,
        index: usize,
        value: Dependency<S, T>,
    ) -> Option<Dependency<S, T>> {
        if index < self.0.len() {
            match self.0.insert_full(value) {
                (_, true) => return self.0.swap_remove_index(index),
                (idx, false) if idx != index => return self.0.shift_remove_index(index),
                _ => (),
            }
        }

        None
    }

    /// Replace a `Dependency` for a given index in a `DependencySet`, returning the replaced value.
    ///
    /// This removes the element at the given index if its replacement value already exists by
    /// swapping it with the last element of the set and popping it off. **This perturbs the
    /// position of what used to be the last element!**
    pub fn swap_replace_index(
        &mut self,
        index: usize,
        value: Dependency<S, T>,
    ) -> Option<Dependency<S, T>> {
        if index < self.0.len() {
            match self.0.insert_full(value) {
                (_, true) => return self.0.swap_remove_index(index),
                (idx, false) if idx != index => return self.0.swap_remove_index(index),
                _ => (),
            }
        }

        None
    }

    /// Return the number of `Dependency` objects a `DependencySet` contains.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.0.is_disjoint(&other.0)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        self.0.is_superset(&other.0)
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

impl<S: Stringable, T: Ordered> Default for DependencySet<S, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Stringable, T: Display + Ordered> Display for DependencySet<S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", p!(self))
    }
}

impl<S: Stringable, T: Ordered> FromIterator<Dependency<S, T>> for DependencySet<S, T> {
    fn from_iter<I: IntoIterator<Item = Dependency<S, T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, T: Ordered + 'a> FromIterator<&'a Dependency<String, T>>
    for DependencySet<&'a str, &'a T>
{
    fn from_iter<I: IntoIterator<Item = &'a Dependency<String, T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().map(|d| d.to_ref()).collect())
    }
}

impl FromStr for DependencySet<String, Dep<String>> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let eapi = Default::default();
        parse::package_dependency_set(s, eapi)
    }
}

impl<S: Stringable, T: Ordered> BitAnd<&Self> for DependencySet<S, T> {
    type Output = Self;

    fn bitand(mut self, other: &Self) -> Self::Output {
        self &= other;
        self
    }
}

impl<S: Stringable, T: Ordered> BitAndAssign<&Self> for DependencySet<S, T> {
    fn bitand_assign(&mut self, other: &Self) {
        self.0 &= &other.0;
    }
}

impl<S: Stringable, T: Ordered> BitOr<&Self> for DependencySet<S, T> {
    type Output = Self;

    fn bitor(mut self, other: &Self) -> Self::Output {
        self |= other;
        self
    }
}

impl<S: Stringable, T: Ordered> BitOrAssign<&Self> for DependencySet<S, T> {
    fn bitor_assign(&mut self, other: &Self) {
        self.0 |= &other.0;
    }
}

impl<S: Stringable, T: Ordered> BitXor<&Self> for DependencySet<S, T> {
    type Output = Self;

    fn bitxor(mut self, other: &Self) -> Self::Output {
        self ^= other;
        self
    }
}

impl<S: Stringable, T: Ordered> BitXorAssign<&Self> for DependencySet<S, T> {
    fn bitxor_assign(&mut self, other: &Self) {
        self.0 ^= &other.0;
    }
}

impl<S: Stringable, T: Ordered> Sub<&Self> for DependencySet<S, T> {
    type Output = Self;

    fn sub(mut self, other: &Self) -> Self::Output {
        self -= other;
        self
    }
}

impl<S: Stringable, T: Ordered> SubAssign<&Self> for DependencySet<S, T> {
    fn sub_assign(&mut self, other: &Self) {
        self.0 -= &other.0;
    }
}

impl<S: Stringable, T: Ordered> Contains<&Dependency<S, T>> for DependencySet<S, T> {
    fn contains(&self, dep: &Dependency<S, T>) -> bool {
        self.0.contains(dep)
    }
}

impl<S: Stringable, T: Ordered> Contains<&T> for DependencySet<S, T> {
    fn contains(&self, obj: &T) -> bool {
        self.iter_flatten().any(|x| x == obj)
    }
}

impl<'a, S: Stringable + 'a, T: Ordered> Evaluate<'a, S> for &'a DependencySet<String, T> {
    type Evaluated = DependencySet<&'a str, &'a T>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = Dependency<&'a str, &'a T>;
    type IntoIterEvaluate = IterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IterEvaluate {
            q: self.0.iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for &'a DependencySet<String, T> {
    type Evaluated = DependencySet<&'a str, &'a T>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = Dependency<&'a str, &'a T>;
    type IntoIterEvaluateForce = IterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IterEvaluateForce {
            q: self.0.iter().collect(),
            force,
        }
    }
}

impl<'a, S: Stringable + 'a, T: Ordered> Evaluate<'a, S> for DependencySet<&'a str, &'a T> {
    type Evaluated = DependencySet<&'a str, &'a T>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = Dependency<&'a str, &'a T>;
    type IntoIterEvaluate = IntoIterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IntoIterEvaluate {
            q: self.0.into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for DependencySet<&'a str, &'a T> {
    type Evaluated = DependencySet<&'a str, &'a T>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = Dependency<&'a str, &'a T>;
    type IntoIterEvaluateForce = IntoIterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IntoIterEvaluateForce {
            q: self.0.into_iter().collect(),
            force,
        }
    }
}

impl<T: Ordered> IntoOwned for DependencySet<&str, &T> {
    type Owned = DependencySet<String, T>;

    fn into_owned(self) -> Self::Owned {
        self.into_iter().map(|d| d.into_owned()).collect()
    }
}

#[derive(Debug)]
pub struct Iter<'a, S: Stringable, T: Ordered>(Deque<&'a Dependency<S, T>>);

impl<'a, S: Stringable, T: Ordered> FromIterator<&'a Dependency<S, T>> for Iter<'a, S, T> {
    fn from_iter<I: IntoIterator<Item = &'a Dependency<S, T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, S: Stringable, T: Ordered> Iterator for Iter<'a, S, T> {
    type Item = &'a Dependency<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front()
    }
}

impl<'a, S: Stringable, T: Ordered> DoubleEndedIterator for Iter<'a, S, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.pop_back()
    }
}

impl<'a, S: Stringable, T: Ordered> IntoIterator for &'a DependencySet<S, T> {
    type Item = &'a Dependency<S, T>;
    type IntoIter = Iter<'a, S, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().collect()
    }
}

impl<'a, S: Stringable, T: Ordered> Flatten for &'a DependencySet<S, T> {
    type Item = &'a T;
    type IntoIterFlatten = IterFlatten<'a, S, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IterFlatten(self.0.iter().collect())
    }
}

impl<'a, S: Stringable, T: Ordered> Recursive for &'a DependencySet<S, T> {
    type Item = &'a Dependency<S, T>;
    type IntoIterRecursive = IterRecursive<'a, S, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IterRecursive(self.0.iter().collect())
    }
}

impl<'a, S: Stringable, T: Ordered> Conditionals for &'a DependencySet<S, T> {
    type Item = &'a UseDep<S>;
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
pub struct IterEvaluate<'a, S: Stringable, T: Ordered> {
    q: Deque<&'a Dependency<String, T>>,
    options: &'a IndexSet<S>,
}

impl<'a, S: Stringable, T: Ordered> Iterator for IterEvaluate<'a, S, T> {
    type Item = Dependency<&'a str, &'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval!(AllOf, vals, self.options),
                AnyOf(vals) => iter_eval!(AnyOf, vals, self.options),
                ExactlyOneOf(vals) => iter_eval!(ExactlyOneOf, vals, self.options),
                AtMostOneOf(vals) => iter_eval!(AtMostOneOf, vals, self.options),
                UseConditional(u, vals) => {
                    if u.matches(self.options) {
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
    q: Deque<&'a Dependency<String, T>>,
    force: bool,
}

impl<'a, T: Ordered> Iterator for IterEvaluateForce<'a, T> {
    type Item = Dependency<&'a str, &'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval_force!(AllOf, vals, self.force),
                AnyOf(vals) => iter_eval_force!(AnyOf, vals, self.force),
                ExactlyOneOf(vals) => iter_eval_force!(ExactlyOneOf, vals, self.force),
                AtMostOneOf(vals) => iter_eval_force!(AtMostOneOf, vals, self.force),
                UseConditional(_, vals) => {
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
pub struct IterFlatten<'a, S: Stringable, T: Ordered>(Deque<&'a Dependency<S, T>>);

impl<'a, S: Stringable, T: Ordered> Iterator for IterFlatten<'a, S, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                UseConditional(_, vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIter<S: Stringable, T: Ordered>(Deque<Dependency<S, T>>);

impl<S: Stringable, T: Ordered> FromIterator<Dependency<S, T>> for IntoIter<S, T> {
    fn from_iter<I: IntoIterator<Item = Dependency<S, T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<S: Stringable, T: Ordered> Iterator for IntoIter<S, T> {
    type Item = Dependency<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front()
    }
}

impl<S: Stringable, T: Ordered> DoubleEndedIterator for IntoIter<S, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.pop_back()
    }
}

impl<S: Stringable, T: Ordered> IntoIterator for DependencySet<S, T> {
    type Item = Dependency<S, T>;
    type IntoIter = IntoIter<S, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().collect()
    }
}

impl<S: Stringable, T: Ordered> Flatten for DependencySet<S, T> {
    type Item = T;
    type IntoIterFlatten = IntoIterFlatten<S, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IntoIterFlatten(self.0.into_iter().collect())
    }
}

impl<S: Stringable, T: Ordered> Recursive for DependencySet<S, T> {
    type Item = Dependency<S, T>;
    type IntoIterRecursive = IntoIterRecursive<S, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IntoIterRecursive(self.0.into_iter().collect())
    }
}

impl<S: Stringable, T: Ordered> Conditionals for DependencySet<S, T> {
    type Item = UseDep<S>;
    type IntoIterConditionals = IntoIterConditionals<S, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IntoIterConditionals(self.0.into_iter().collect())
    }
}

#[derive(Debug)]
pub struct IntoIterEvaluate<'a, S: Stringable, T: Ordered> {
    q: Deque<Dependency<&'a str, &'a T>>,
    options: &'a IndexSet<S>,
}

impl<'a, S: Stringable, T: Ordered> Iterator for IntoIterEvaluate<'a, S, T> {
    type Item = Dependency<&'a str, &'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval!(AllOf, vals, self.options),
                AnyOf(vals) => iter_eval!(AnyOf, vals, self.options),
                ExactlyOneOf(vals) => iter_eval!(ExactlyOneOf, vals, self.options),
                AtMostOneOf(vals) => iter_eval!(AtMostOneOf, vals, self.options),
                UseConditional(u, vals) => {
                    if u.matches(self.options) {
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
    q: Deque<Dependency<&'a str, &'a T>>,
    force: bool,
}

impl<'a, T: Ordered> Iterator for IntoIterEvaluateForce<'a, T> {
    type Item = Dependency<&'a str, &'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => iter_eval_force!(AllOf, vals, self.force),
                AnyOf(vals) => iter_eval_force!(AnyOf, vals, self.force),
                ExactlyOneOf(vals) => iter_eval_force!(ExactlyOneOf, vals, self.force),
                AtMostOneOf(vals) => iter_eval_force!(AtMostOneOf, vals, self.force),
                UseConditional(_, vals) => {
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
pub struct IntoIterFlatten<S: Stringable, T: Ordered>(Deque<Dependency<S, T>>);

impl<S: Stringable, T: Ordered> Iterator for IntoIterFlatten<S, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AnyOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AtMostOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                UseConditional(_, vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IterRecursive<'a, S: Stringable, T: Ordered>(Deque<&'a Dependency<S, T>>);

impl<'a, S: Stringable, T: Ordered> Iterator for IterRecursive<'a, S, T> {
    type Item = &'a Dependency<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        let val = self.0.pop_front();
        if let Some(dep) = val {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                UseConditional(_, vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
            }
        }

        val
    }
}

#[derive(Debug)]
pub struct IntoIterRecursive<S: Stringable, T: Ordered>(Deque<Dependency<S, T>>);

impl<S: Stringable, T: Ordered> Iterator for IntoIterRecursive<S, T> {
    type Item = Dependency<S, T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        let val = self.0.pop_front();
        if let Some(dep) = &val {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
                AnyOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
                ExactlyOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
                AtMostOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
                UseConditional(_, vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
            }
        }

        val
    }
}

#[derive(Debug)]
pub struct IterConditionals<'a, S: Stringable, T: Ordered>(Deque<&'a Dependency<S, T>>);

impl<'a, S: Stringable, T: Ordered> Iterator for IterConditionals<'a, S, T> {
    type Item = &'a UseDep<S>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                UseConditional(u, vals) => {
                    self.0.extend_left(vals.iter().map(AsRef::as_ref));
                    return Some(u);
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterConditionals<S: Stringable, T: Ordered>(Deque<Dependency<S, T>>);

impl<S: Stringable, T: Ordered> Iterator for IntoIterConditionals<S, T> {
    type Item = UseDep<S>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AnyOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AtMostOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                UseConditional(u, vals) => {
                    self.0.extend_left(vals.into_iter().map(|x| *x));
                    return Some(u);
                }
            }
        }
        None
    }
}

impl Restriction<&Dependency<String, Dep<String>>> for BaseRestrict {
    fn matches(&self, val: &Dependency<String, Dep<String>>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DependencySet<String, Dep<String>>> for BaseRestrict {
    fn matches(&self, val: &DependencySet<String, Dep<String>>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_contains() {
        let dep = Dep::try_new("cat/pkg").unwrap();
        let spec = Dependency::from_str("cat/pkg").unwrap();
        for s in ["( cat/pkg )", "u? ( cat/pkg )"] {
            let d: Dependency<String, Dep<_>> = s.parse().unwrap();
            assert!(d.contains(&dep), "{d} doesn't contain {dep}");
            assert!(!d.contains(&d), "{d} contains itself");
            assert!(d.contains(&spec), "{d} doesn't contain {spec}");
        }
    }

    #[test]
    fn dependency_sort() {
        // dependencies
        for (s, expected) in [
            ("a/b", "a/b"),
            ("( c/d a/b )", "( a/b c/d )"),
            ("|| ( c/d a/b )", "|| ( c/d a/b )"),
            ("u? ( c/d a/b )", "u? ( a/b c/d )"),
            ("!u? ( c/d a/b )", "!u? ( a/b c/d )"),
        ] {
            let mut spec: Dependency<String, Dep<_>> = s.parse().unwrap();
            spec.sort();
            assert_eq!(spec.to_string(), expected);
        }

        // REQUIRED_USE
        let eapi = Default::default();
        for (s, expected) in [
            ("a", "a"),
            ("!a", "!a"),
            ("( b a )", "( a b )"),
            ("( b !a )", "( b !a )"),
            ("|| ( b a )", "|| ( b a )"),
            ("^^ ( b a )", "^^ ( b a )"),
            ("?? ( b a )", "?? ( b a )"),
            ("u? ( b a )", "u? ( a b )"),
            ("!u? ( b a )", "!u? ( a b )"),
        ] {
            let mut spec = parse::required_use_dependency(s, eapi).unwrap();
            spec.sort();
            assert_eq!(spec.to_string(), expected);
        }
    }

    #[test]
    fn dependency_set_contains() {
        let dep = Dep::try_new("cat/pkg").unwrap();
        let spec = Dependency::from_str("cat/pkg").unwrap();
        for s in ["cat/pkg", "a/b cat/pkg"] {
            let set: DependencySet<String, Dep<_>> = s.parse().unwrap();
            assert!(set.contains(&dep), "{set} doesn't contain {dep}");
            assert!(set.contains(&spec), "{set} doesn't contain {spec}");
        }
    }

    #[test]
    fn dependency_set_sort() {
        // dependencies
        for (s, expected) in [
            ("c/d a/b", "a/b c/d"),
            ("( c/d a/b ) z/z", "z/z ( a/b c/d )"),
            ("|| ( c/d a/b ) z/z", "z/z || ( c/d a/b )"),
            ("u? ( c/d a/b ) z/z", "z/z u? ( a/b c/d )"),
            ("!u? ( c/d a/b ) z/z", "z/z !u? ( a/b c/d )"),
        ] {
            let mut set: DependencySet<String, Dep<_>> = s.parse().unwrap();
            set.sort();
            assert_eq!(set.to_string(), expected);
        }

        // REQUIRED_USE
        let eapi = Default::default();
        for (s, expected) in [
            ("b a", "a b"),
            ("b !a", "b !a"),
            ("( b a ) z", "z ( a b )"),
            ("( b !a ) z", "z ( b !a )"),
            ("|| ( b a ) z", "z || ( b a )"),
            ("^^ ( b a ) z", "z ^^ ( b a )"),
            ("?? ( b a ) z", "z ?? ( b a )"),
            ("u? ( b a ) z", "z u? ( a b )"),
            ("!u? ( b a ) z", "z !u? ( a b )"),
        ] {
            let mut set = parse::required_use_dependency_set(s, eapi).unwrap();
            set.sort();
            assert_eq!(set.to_string(), expected);
        }
    }
}
