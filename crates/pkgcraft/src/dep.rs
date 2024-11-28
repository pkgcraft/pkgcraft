use std::borrow::Borrow;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Sub, SubAssign,
};

use indexmap::IndexSet;
use itertools::Itertools;

use crate::eapi::Eapi;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::{Contains, IntoOwned, ToRef};
use crate::types::{Deque, Ordered, OrderedSet, SortedSet};

pub mod cpn;
pub mod cpv;
pub mod parse;
pub mod pkg;
pub mod uri;
pub mod use_dep;
pub mod version;

pub use cpn::Cpn;
pub use cpv::{Cpv, CpvOrDep};
pub use pkg::{Blocker, Dep, DepField, Slot, SlotDep, SlotOperator};
pub use uri::Uri;
pub use use_dep::{UseDep, UseDepKind};
pub use version::{Operator, Revision, Version};

pub trait Stringable:
    Debug + Display + Default + Ord + Clone + Hash + Borrow<str> + AsRef<str>
{
}

impl<T> Stringable for T where
    T: Debug + Display + Default + Ord + Clone + Hash + Borrow<str> + AsRef<str>
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
pub enum Dependency<T: Ordered> {
    /// Enabled dependency.
    Enabled(T),
    /// Disabled dependency (REQUIRED_USE only).
    Disabled(T),
    /// All of a given dependency set.
    AllOf(SortedSet<Box<Dependency<T>>>),
    /// Any of a given dependency set.
    AnyOf(OrderedSet<Box<Dependency<T>>>),
    /// Exactly one of a given dependency set (REQUIRED_USE only).
    ExactlyOneOf(OrderedSet<Box<Dependency<T>>>),
    /// At most one of a given dependency set (REQUIRED_USE only).
    AtMostOneOf(OrderedSet<Box<Dependency<T>>>),
    /// Conditional dependency.
    Conditional(UseDep, SortedSet<Box<Dependency<T>>>),
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

impl<T: Ordered> IntoOwned for Dependency<&T> {
    type Owned = Dependency<T>;

    fn into_owned(self) -> Self::Owned {
        use Dependency::*;
        match self {
            Enabled(val) => Enabled(val.clone()),
            Disabled(val) => Disabled(val.clone()),
            AllOf(vals) => AllOf(box_owned!(vals)),
            AnyOf(vals) => AnyOf(box_owned!(vals)),
            ExactlyOneOf(vals) => ExactlyOneOf(box_owned!(vals)),
            AtMostOneOf(vals) => AtMostOneOf(box_owned!(vals)),
            Conditional(u, vals) => Conditional(u.clone(), box_owned!(vals)),
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

impl<'a, T: Ordered + 'a> ToRef<'a> for Dependency<T> {
    type Ref = Dependency<&'a T>;

    fn to_ref(&'a self) -> Self::Ref {
        use Dependency::*;
        match self {
            Enabled(ref val) => Enabled(val),
            Disabled(ref val) => Disabled(val),
            AllOf(ref vals) => AllOf(box_ref!(vals)),
            AnyOf(ref vals) => AnyOf(box_ref!(vals)),
            ExactlyOneOf(ref vals) => ExactlyOneOf(box_ref!(vals)),
            AtMostOneOf(ref vals) => AtMostOneOf(box_ref!(vals)),
            // TODO: replace clone with borrowed ref when dep evaluation is reworked
            Conditional(ref u, ref vals) => Conditional(u.clone(), box_ref!(vals)),
        }
    }
}

impl<T: Ordered> PartialEq<Dependency<&T>> for Dependency<T> {
    fn eq(&self, other: &Dependency<&T>) -> bool {
        self.to_ref() == *other
    }
}

impl<T: Ordered> PartialEq<Dependency<T>> for Dependency<&T> {
    fn eq(&self, other: &Dependency<T>) -> bool {
        other == self
    }
}

impl<T: Ordered> Dependency<T> {
    /// Return the `Dependency` for a given index if it exists.
    pub fn get_index(&self, index: usize) -> Option<&Dependency<T>> {
        use Dependency::*;
        match self {
            Enabled(_) | Disabled(_) => None,
            AllOf(vals) => vals.get_index(index).map(AsRef::as_ref),
            AnyOf(vals) => vals.get_index(index).map(AsRef::as_ref),
            ExactlyOneOf(vals) => vals.get_index(index).map(AsRef::as_ref),
            AtMostOneOf(vals) => vals.get_index(index).map(AsRef::as_ref),
            Conditional(_, vals) => vals.get_index(index).map(AsRef::as_ref),
        }
    }

    /// Return true if a Dependency is empty, otherwise false.
    pub fn is_empty(&self) -> bool {
        use Dependency::*;
        match self {
            Enabled(_) | Disabled(_) => false,
            AllOf(vals) => vals.is_empty(),
            AnyOf(vals) => vals.is_empty(),
            ExactlyOneOf(vals) => vals.is_empty(),
            AtMostOneOf(vals) => vals.is_empty(),
            Conditional(_, vals) => vals.is_empty(),
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
            Conditional(_, vals) => vals.len(),
        }
    }

    pub fn iter(&self) -> Iter<T> {
        self.into_iter()
    }

    pub fn iter_flatten(&self) -> IterFlatten<T> {
        self.into_iter_flatten()
    }

    pub fn iter_recursive(&self) -> IterRecursive<T> {
        self.into_iter_recursive()
    }

    pub fn iter_conditionals(&self) -> IterConditionals<T> {
        self.into_iter_conditionals()
    }

    /// Recursively sort a `Dependency`.
    pub fn sort(&mut self) {
        match self {
            Self::AllOf(vals) => *vals = sort_set!(vals).collect(),
            Self::Conditional(_, vals) => *vals = sort_set!(vals).collect(),
            _ => (),
        }
    }
}

impl Dependency<Dep> {
    pub fn package(s: &str, eapi: &'static Eapi) -> crate::Result<Self> {
        parse::package_dependency(s, eapi)
    }
}

impl Dependency<String> {
    pub fn license(s: &str) -> crate::Result<Self> {
        parse::license_dependency(s)
    }

    pub fn properties(s: &str) -> crate::Result<Self> {
        parse::properties_dependency(s)
    }

    pub fn required_use(s: &str) -> crate::Result<Self> {
        parse::required_use_dependency(s)
    }

    pub fn restrict(s: &str) -> crate::Result<Self> {
        parse::restrict_dependency(s)
    }
}

impl Dependency<Uri> {
    pub fn src_uri(s: &str) -> crate::Result<Self> {
        parse::src_uri_dependency(s)
    }
}

impl<T1: Ordered, T2: Ordered> Contains<&Dependency<T1>> for Dependency<T2>
where
    Dependency<T2>: PartialEq<Dependency<T1>>,
{
    fn contains(&self, dep: &Dependency<T1>) -> bool {
        self.iter_recursive().any(|x| x == dep)
    }
}

impl<T: Ordered> Contains<&UseDep> for Dependency<T> {
    fn contains(&self, obj: &UseDep) -> bool {
        self.iter_conditionals().any(|x| x == obj)
    }
}

impl<S: AsRef<str>, T: Ordered + AsRef<str>> Contains<S> for Dependency<T> {
    fn contains(&self, obj: S) -> bool {
        self.iter_flatten().any(|x| x.as_ref() == obj.as_ref())
    }
}

// Merge with AsRef<str> implementation if Dep ever supports that.
impl<S: AsRef<str>> Contains<S> for Dependency<Dep> {
    fn contains(&self, obj: S) -> bool {
        self.iter_flatten().any(|x| x.to_string() == obj.as_ref())
    }
}

// Merge with AsRef<str> implementation if Dep ever supports that.
impl<S: AsRef<str>> Contains<S> for Dependency<&Dep> {
    fn contains(&self, obj: S) -> bool {
        self.iter_flatten().any(|x| x.to_string() == obj.as_ref())
    }
}

impl Contains<&Dep> for Dependency<Dep> {
    fn contains(&self, obj: &Dep) -> bool {
        self.iter_flatten().any(|x| x == obj)
    }
}

impl Contains<&Dep> for Dependency<&Dep> {
    fn contains(&self, obj: &Dep) -> bool {
        self.iter_flatten().any(|x| *x == obj)
    }
}

impl<'a, T: Ordered> IntoIterator for &'a Dependency<T> {
    type Item = &'a Dependency<T>;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        use Dependency::*;
        match self {
            Enabled(_) | Disabled(_) => [].into_iter().collect(),
            AllOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            AnyOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            ExactlyOneOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            AtMostOneOf(vals) => vals.iter().map(AsRef::as_ref).collect(),
            Conditional(_, vals) => vals.iter().map(AsRef::as_ref).collect(),
        }
    }
}

impl<'a, S: Stringable + 'a, T: Ordered> Evaluate<'a, S> for &'a Dependency<T> {
    type Evaluated = SortedSet<Dependency<&'a T>>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = Dependency<&'a T>;
    type IntoIterEvaluate = IterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IterEvaluate {
            q: [self].into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for &'a Dependency<T> {
    type Evaluated = SortedSet<Dependency<&'a T>>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = Dependency<&'a T>;
    type IntoIterEvaluateForce = IterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IterEvaluateForce {
            q: [self].into_iter().collect(),
            force,
        }
    }
}

impl<'a, S: Stringable + 'a, T: Ordered> Evaluate<'a, S> for Dependency<&'a T> {
    type Evaluated = SortedSet<Dependency<&'a T>>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = Dependency<&'a T>;
    type IntoIterEvaluate = IntoIterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IntoIterEvaluate {
            q: [self].into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for Dependency<&'a T> {
    type Evaluated = SortedSet<Dependency<&'a T>>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = Dependency<&'a T>;
    type IntoIterEvaluateForce = IntoIterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IntoIterEvaluateForce {
            q: [self].into_iter().collect(),
            force,
        }
    }
}

impl<T: Ordered> IntoIterator for Dependency<T> {
    type Item = Dependency<T>;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        use Dependency::*;
        match self {
            Enabled(_) | Disabled(_) => [].into_iter().collect(),
            AllOf(vals) => vals.into_iter().map(|x| *x).collect(),
            AnyOf(vals) => vals.into_iter().map(|x| *x).collect(),
            ExactlyOneOf(vals) => vals.into_iter().map(|x| *x).collect(),
            AtMostOneOf(vals) => vals.into_iter().map(|x| *x).collect(),
            Conditional(_, vals) => vals.into_iter().map(|x| *x).collect(),
        }
    }
}

impl<'a, T: Ordered> Flatten for &'a Dependency<T> {
    type Item = &'a T;
    type IntoIterFlatten = IterFlatten<'a, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IterFlatten([self].into_iter().collect())
    }
}

impl<'a, T: Ordered> Recursive for &'a Dependency<T> {
    type Item = &'a Dependency<T>;
    type IntoIterRecursive = IterRecursive<'a, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IterRecursive([self].into_iter().collect())
    }
}

impl<'a, T: Ordered> Conditionals for &'a Dependency<T> {
    type Item = &'a UseDep;
    type IntoIterConditionals = IterConditionals<'a, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IterConditionals([self].into_iter().collect())
    }
}

impl<T: Ordered> Flatten for Dependency<T> {
    type Item = T;
    type IntoIterFlatten = IntoIterFlatten<T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IntoIterFlatten([self].into_iter().collect())
    }
}

impl<T: Ordered> Recursive for Dependency<T> {
    type Item = Dependency<T>;
    type IntoIterRecursive = IntoIterRecursive<T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IntoIterRecursive([self].into_iter().collect())
    }
}

impl<T: Ordered> Conditionals for Dependency<T> {
    type Item = UseDep;
    type IntoIterConditionals = IntoIterConditionals<T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IntoIterConditionals([self].into_iter().collect())
    }
}

impl<T: Display + Ordered> Display for Dependency<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Dependency::*;
        match self {
            Enabled(val) => write!(f, "{val}"),
            Disabled(val) => write!(f, "!{val}"),
            AllOf(vals) => write!(f, "( {} )", p!(vals)),
            AnyOf(vals) => write!(f, "|| ( {} )", p!(vals)),
            ExactlyOneOf(vals) => write!(f, "^^ ( {} )", p!(vals)),
            AtMostOneOf(vals) => write!(f, "?? ( {} )", p!(vals)),
            Conditional(u, vals) => write!(f, "{u} ( {} )", p!(vals)),
        }
    }
}

/// Set of dependency objects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DependencySet<T: Ordered>(SortedSet<Dependency<T>>);

impl<T: Ordered> PartialEq<DependencySet<&T>> for DependencySet<T> {
    fn eq(&self, other: &DependencySet<&T>) -> bool {
        self.to_ref() == *other
    }
}

impl<T: Ordered> PartialEq<DependencySet<T>> for DependencySet<&T> {
    fn eq(&self, other: &DependencySet<T>) -> bool {
        other == self
    }
}

impl<T: Ordered> Deref for DependencySet<T> {
    type Target = SortedSet<Dependency<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Ordered> DerefMut for DependencySet<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Ordered> Default for DependencySet<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Ordered> DependencySet<T> {
    /// Construct a new, empty `DependencySet`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Recursively sort a `DependencySet`.
    pub fn sort_recursive(&mut self) {
        self.0 = sort_set!(self.0).collect();
    }

    /// Replace a `Dependency` with another `Dependency`, returning the replaced value.
    ///
    /// This removes the given element if its replacement value already exists by shifting all of
    /// the elements that follow it, preserving their relative order. **This perturbs the index of
    /// all of those elements!**
    pub fn shift_replace(
        &mut self,
        key: &Dependency<T>,
        value: Dependency<T>,
    ) -> Option<Dependency<T>> {
        self.get_index_of(key)
            .and_then(|i| self.shift_replace_index(i, value))
    }

    /// Replace a `Dependency` with another `Dependency`, returning the replaced value.
    ///
    /// This removes the given element if its replacement value already exists by swapping it with
    /// the last element of the set and popping it off. **This perturbs the position of what used
    /// to be the last element!**
    pub fn swap_replace(
        &mut self,
        key: &Dependency<T>,
        value: Dependency<T>,
    ) -> Option<Dependency<T>> {
        self.get_index_of(key)
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
        value: Dependency<T>,
    ) -> Option<Dependency<T>> {
        if index < self.len() {
            match self.insert_full(value) {
                (_, true) => return self.swap_remove_index(index),
                (idx, false) if idx != index => return self.shift_remove_index(index),
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
        value: Dependency<T>,
    ) -> Option<Dependency<T>> {
        if index < self.len() {
            match self.insert_full(value) {
                (_, true) => return self.swap_remove_index(index),
                (idx, false) if idx != index => return self.swap_remove_index(index),
                _ => (),
            }
        }

        None
    }

    pub fn iter(&self) -> Iter<T> {
        self.into_iter()
    }

    pub fn iter_flatten(&self) -> IterFlatten<T> {
        self.into_iter_flatten()
    }

    pub fn iter_recursive(&self) -> IterRecursive<T> {
        self.into_iter_recursive()
    }

    pub fn iter_conditionals(&self) -> IterConditionals<T> {
        self.into_iter_conditionals()
    }
}

impl DependencySet<Dep> {
    pub fn package(s: &str, eapi: &'static Eapi) -> crate::Result<Self> {
        parse::package_dependency_set(s, eapi)
    }
}

impl DependencySet<String> {
    pub fn license(s: &str) -> crate::Result<Self> {
        parse::license_dependency_set(s)
    }

    pub fn properties(s: &str) -> crate::Result<Self> {
        parse::properties_dependency_set(s)
    }

    pub fn required_use(s: &str) -> crate::Result<Self> {
        parse::required_use_dependency_set(s)
    }

    pub fn restrict(s: &str) -> crate::Result<Self> {
        parse::restrict_dependency_set(s)
    }
}

impl DependencySet<Uri> {
    pub fn src_uri(s: &str) -> crate::Result<Self> {
        parse::src_uri_dependency_set(s)
    }
}

impl<T: Display + Ordered> Display for DependencySet<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", p!(self))
    }
}

impl<T: Ordered> FromIterator<Dependency<T>> for DependencySet<T> {
    fn from_iter<I: IntoIterator<Item = Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, T: Ordered + 'a> FromIterator<&'a Dependency<T>> for DependencySet<&'a T> {
    fn from_iter<I: IntoIterator<Item = &'a Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().map(|d| d.to_ref()).collect())
    }
}

impl<T: Ordered> BitAnd<&Self> for DependencySet<T> {
    type Output = Self;

    fn bitand(mut self, other: &Self) -> Self::Output {
        self &= other;
        self
    }
}

impl<T: Ordered> BitAndAssign<&Self> for DependencySet<T> {
    fn bitand_assign(&mut self, other: &Self) {
        self.0 &= &other.0;
    }
}

impl<T: Ordered> BitOr<&Self> for DependencySet<T> {
    type Output = Self;

    fn bitor(mut self, other: &Self) -> Self::Output {
        self |= other;
        self
    }
}

impl<T: Ordered> BitOrAssign<&Self> for DependencySet<T> {
    fn bitor_assign(&mut self, other: &Self) {
        self.0 |= &other.0;
    }
}

impl<T: Ordered> BitXor<&Self> for DependencySet<T> {
    type Output = Self;

    fn bitxor(mut self, other: &Self) -> Self::Output {
        self ^= other;
        self
    }
}

impl<T: Ordered> BitXorAssign<&Self> for DependencySet<T> {
    fn bitxor_assign(&mut self, other: &Self) {
        self.0 ^= &other.0;
    }
}

impl<T: Ordered> Sub<&Self> for DependencySet<T> {
    type Output = Self;

    fn sub(mut self, other: &Self) -> Self::Output {
        self -= other;
        self
    }
}

impl<T: Ordered> SubAssign<&Self> for DependencySet<T> {
    fn sub_assign(&mut self, other: &Self) {
        self.0 -= &other.0;
    }
}

impl<T1: Ordered, T2: Ordered> Contains<&Dependency<T1>> for DependencySet<T2>
where
    Dependency<T2>: PartialEq<Dependency<T1>>,
{
    fn contains(&self, dep: &Dependency<T1>) -> bool {
        self.iter_recursive().any(|x| x == dep)
    }
}

impl<T: Ordered> Contains<&UseDep> for DependencySet<T> {
    fn contains(&self, obj: &UseDep) -> bool {
        self.iter_conditionals().any(|x| x == obj)
    }
}

impl<S: AsRef<str>, T: Ordered + AsRef<str>> Contains<S> for DependencySet<T> {
    fn contains(&self, obj: S) -> bool {
        self.iter_flatten().any(|x| x.as_ref() == obj.as_ref())
    }
}

// Merge with AsRef<str> implementation if Dep ever supports that.
impl<S: AsRef<str>> Contains<S> for DependencySet<Dep> {
    fn contains(&self, obj: S) -> bool {
        self.iter_flatten().any(|x| x.to_string() == obj.as_ref())
    }
}

// Merge with AsRef<str> implementation if Dep ever supports that.
impl<S: AsRef<str>> Contains<S> for DependencySet<&Dep> {
    fn contains(&self, obj: S) -> bool {
        self.iter_flatten().any(|x| x.to_string() == obj.as_ref())
    }
}

impl Contains<&Dep> for DependencySet<Dep> {
    fn contains(&self, obj: &Dep) -> bool {
        self.iter_flatten().any(|x| x == obj)
    }
}

impl Contains<&Dep> for DependencySet<&Dep> {
    fn contains(&self, obj: &Dep) -> bool {
        self.iter_flatten().any(|x| *x == obj)
    }
}

impl<'a, S: Stringable + 'a, T: Ordered> Evaluate<'a, S> for &'a DependencySet<T> {
    type Evaluated = DependencySet<&'a T>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = Dependency<&'a T>;
    type IntoIterEvaluate = IterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IterEvaluate {
            q: self.0.iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for &'a DependencySet<T> {
    type Evaluated = DependencySet<&'a T>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = Dependency<&'a T>;
    type IntoIterEvaluateForce = IterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IterEvaluateForce {
            q: self.0.iter().collect(),
            force,
        }
    }
}

impl<'a, S: Stringable + 'a, T: Ordered> Evaluate<'a, S> for DependencySet<&'a T> {
    type Evaluated = DependencySet<&'a T>;
    fn evaluate(self, options: &'a IndexSet<S>) -> Self::Evaluated {
        self.into_iter_evaluate(options).collect()
    }

    type Item = Dependency<&'a T>;
    type IntoIterEvaluate = IntoIterEvaluate<'a, S, T>;
    fn into_iter_evaluate(self, options: &'a IndexSet<S>) -> Self::IntoIterEvaluate {
        IntoIterEvaluate {
            q: self.0.into_iter().collect(),
            options,
        }
    }
}

impl<'a, T: Ordered> EvaluateForce for DependencySet<&'a T> {
    type Evaluated = DependencySet<&'a T>;
    fn evaluate_force(self, force: bool) -> Self::Evaluated {
        self.into_iter_evaluate_force(force).collect()
    }

    type Item = Dependency<&'a T>;
    type IntoIterEvaluateForce = IntoIterEvaluateForce<'a, T>;
    fn into_iter_evaluate_force(self, force: bool) -> Self::IntoIterEvaluateForce {
        IntoIterEvaluateForce {
            q: self.0.into_iter().collect(),
            force,
        }
    }
}

impl<T: Ordered> IntoOwned for DependencySet<&T> {
    type Owned = DependencySet<T>;

    fn into_owned(self) -> Self::Owned {
        self.into_iter().map(|d| d.into_owned()).collect()
    }
}

impl<'a, T: Ordered + 'a> ToRef<'a> for DependencySet<T> {
    type Ref = DependencySet<&'a T>;

    fn to_ref(&'a self) -> Self::Ref {
        self.iter().map(|d| d.to_ref()).collect()
    }
}

#[derive(Debug)]
pub struct Iter<'a, T: Ordered>(Deque<&'a Dependency<T>>);

impl<'a, T: Ordered> FromIterator<&'a Dependency<T>> for Iter<'a, T> {
    fn from_iter<I: IntoIterator<Item = &'a Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<'a, T: Ordered> Iterator for Iter<'a, T> {
    type Item = &'a Dependency<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front()
    }
}

impl<T: Ordered> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.pop_back()
    }
}

impl<'a, T: Ordered> IntoIterator for &'a DependencySet<T> {
    type Item = &'a Dependency<T>;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().collect()
    }
}

impl<'a, T: Ordered> Flatten for &'a DependencySet<T> {
    type Item = &'a T;
    type IntoIterFlatten = IterFlatten<'a, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IterFlatten(self.0.iter().collect())
    }
}

impl<'a, T: Ordered> Recursive for &'a DependencySet<T> {
    type Item = &'a Dependency<T>;
    type IntoIterRecursive = IterRecursive<'a, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IterRecursive(self.0.iter().collect())
    }
}

impl<'a, T: Ordered> Conditionals for &'a DependencySet<T> {
    type Item = &'a UseDep;
    type IntoIterConditionals = IterConditionals<'a, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IterConditionals(self.0.iter().collect())
    }
}

macro_rules! box_eval {
    ($vals:expr, $options:expr) => {
        $vals
            .into_iter()
            .flat_map(|d| d.into_iter_evaluate($options))
            .map(|d| Box::new(d))
            .collect()
    };
}

#[derive(Debug)]
pub struct IterEvaluate<'a, S: Stringable, T: Ordered> {
    q: Deque<&'a Dependency<T>>,
    options: &'a IndexSet<S>,
}

impl<'a, S: Stringable, T: Ordered> Iterator for IterEvaluate<'a, S, T> {
    type Item = Dependency<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => {
                    let evaluated = AllOf(box_eval!(vals, self.options));
                    if !evaluated.is_empty() {
                        return Some(evaluated);
                    }
                }
                AnyOf(vals) => {
                    let evaluated = AnyOf(box_eval!(vals, self.options));
                    if !evaluated.is_empty() {
                        return Some(evaluated);
                    }
                }
                ExactlyOneOf(vals) => {
                    let evaluated = ExactlyOneOf(box_eval!(vals, self.options));
                    if !evaluated.is_empty() {
                        return Some(evaluated);
                    }
                }
                AtMostOneOf(vals) => {
                    let evaluated = AtMostOneOf(box_eval!(vals, self.options));
                    if !evaluated.is_empty() {
                        return Some(evaluated);
                    }
                }
                Conditional(u, vals) => {
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
    q: Deque<&'a Dependency<T>>,
    force: bool,
}

impl<'a, T: Ordered> Iterator for IterEvaluateForce<'a, T> {
    type Item = Dependency<&'a T>;

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
                Conditional(_, vals) => {
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
pub struct IterFlatten<'a, T: Ordered>(Deque<&'a Dependency<T>>);

impl<'a, T: Ordered> Iterator for IterFlatten<'a, T> {
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
                Conditional(_, vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
            }
        }
        None
    }
}

impl<T: Ordered> DoubleEndedIterator for IterFlatten<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_back() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend(vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => self.0.extend(vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => self.0.extend(vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => self.0.extend(vals.iter().map(AsRef::as_ref)),
                Conditional(_, vals) => self.0.extend(vals.iter().map(AsRef::as_ref)),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIter<T: Ordered>(Deque<Dependency<T>>);

impl<T: Ordered> FromIterator<Dependency<T>> for IntoIter<T> {
    fn from_iter<I: IntoIterator<Item = Dependency<T>>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<T: Ordered> Iterator for IntoIter<T> {
    type Item = Dependency<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop_front()
    }
}

impl<T: Ordered> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.pop_back()
    }
}

impl<T: Ordered> IntoIterator for DependencySet<T> {
    type Item = Dependency<T>;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().collect()
    }
}

impl<T: Ordered> Flatten for DependencySet<T> {
    type Item = T;
    type IntoIterFlatten = IntoIterFlatten<T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        IntoIterFlatten(self.0.into_iter().collect())
    }
}

impl<T: Ordered> Recursive for DependencySet<T> {
    type Item = Dependency<T>;
    type IntoIterRecursive = IntoIterRecursive<T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        IntoIterRecursive(self.0.into_iter().collect())
    }
}

impl<T: Ordered> Conditionals for DependencySet<T> {
    type Item = UseDep;
    type IntoIterConditionals = IntoIterConditionals<T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        IntoIterConditionals(self.0.into_iter().collect())
    }
}

#[derive(Debug)]
pub struct IntoIterEvaluate<'a, S: Stringable, T: Ordered> {
    q: Deque<Dependency<&'a T>>,
    options: &'a IndexSet<S>,
}

impl<'a, S: Stringable, T: Ordered> Iterator for IntoIterEvaluate<'a, S, T> {
    type Item = Dependency<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.q.pop_front() {
            match dep {
                Enabled(val) => return Some(Enabled(val)),
                Disabled(val) => return Some(Disabled(val)),
                AllOf(vals) => {
                    let evaluated = AllOf(box_eval!(vals, self.options));
                    if !evaluated.is_empty() {
                        return Some(evaluated);
                    }
                }
                AnyOf(vals) => {
                    let evaluated = AnyOf(box_eval!(vals, self.options));
                    if !evaluated.is_empty() {
                        return Some(evaluated);
                    }
                }
                ExactlyOneOf(vals) => {
                    let evaluated = ExactlyOneOf(box_eval!(vals, self.options));
                    if !evaluated.is_empty() {
                        return Some(evaluated);
                    }
                }
                AtMostOneOf(vals) => {
                    let evaluated = AtMostOneOf(box_eval!(vals, self.options));
                    if !evaluated.is_empty() {
                        return Some(evaluated);
                    }
                }
                Conditional(u, vals) => {
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
    q: Deque<Dependency<&'a T>>,
    force: bool,
}

impl<'a, T: Ordered> Iterator for IntoIterEvaluateForce<'a, T> {
    type Item = Dependency<&'a T>;

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
                Conditional(_, vals) => {
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
pub struct IntoIterFlatten<T: Ordered>(Deque<Dependency<T>>);

impl<T: Ordered> Iterator for IntoIterFlatten<T> {
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
                Conditional(_, vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
            }
        }
        None
    }
}

impl<T: Ordered> DoubleEndedIterator for IntoIterFlatten<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_back() {
            match dep {
                Enabled(val) | Disabled(val) => return Some(val),
                AllOf(vals) => self.0.extend(vals.into_iter().map(|x| *x)),
                AnyOf(vals) => self.0.extend(vals.into_iter().map(|x| *x)),
                ExactlyOneOf(vals) => self.0.extend(vals.into_iter().map(|x| *x)),
                AtMostOneOf(vals) => self.0.extend(vals.into_iter().map(|x| *x)),
                Conditional(_, vals) => self.0.extend(vals.into_iter().map(|x| *x)),
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IterRecursive<'a, T: Ordered>(Deque<&'a Dependency<T>>);

impl<'a, T: Ordered> Iterator for IterRecursive<'a, T> {
    type Item = &'a Dependency<T>;

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
                Conditional(_, vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
            }
        }

        val
    }
}

#[derive(Debug)]
pub struct IntoIterRecursive<T: Ordered>(Deque<Dependency<T>>);

impl<T: Ordered> Iterator for IntoIterRecursive<T> {
    type Item = Dependency<T>;

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
                Conditional(_, vals) => self.0.extend_left(vals.into_iter().map(|x| *x.clone())),
            }
        }

        val
    }
}

#[derive(Debug)]
pub struct IterConditionals<'a, T: Ordered>(Deque<&'a Dependency<T>>);

impl<'a, T: Ordered> Iterator for IterConditionals<'a, T> {
    type Item = &'a UseDep;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AnyOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                AtMostOneOf(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                Conditional(u, vals) => {
                    self.0.extend_left(vals.iter().map(AsRef::as_ref));
                    return Some(u);
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct IntoIterConditionals<T: Ordered>(Deque<Dependency<T>>);

impl<T: Ordered> Iterator for IntoIterConditionals<T> {
    type Item = UseDep;

    fn next(&mut self) -> Option<Self::Item> {
        use Dependency::*;
        while let Some(dep) = self.0.pop_front() {
            match dep {
                Enabled(_) | Disabled(_) => (),
                AllOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AnyOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                ExactlyOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                AtMostOneOf(vals) => self.0.extend_left(vals.into_iter().map(|x| *x)),
                Conditional(u, vals) => {
                    self.0.extend_left(vals.into_iter().map(|x| *x));
                    return Some(u);
                }
            }
        }
        None
    }
}

impl Restriction<&Dependency<Dep>> for BaseRestrict {
    fn matches(&self, val: &Dependency<Dep>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DependencySet<Dep>> for BaseRestrict {
    fn matches(&self, val: &DependencySet<Dep>) -> bool {
        crate::restrict::restrict_match! {self, val,
            Self::Dep(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn dep_variants() {
        // Dependency<Dep>
        Dependency::package("a/b", Default::default()).unwrap();
        // Dependency<String>
        Dependency::license("a").unwrap();
        Dependency::properties("a").unwrap();
        Dependency::required_use("a").unwrap();
        Dependency::restrict("a").unwrap();
        // Dependency<Uri>
        Dependency::src_uri("https://uri").unwrap();
    }

    #[test]
    fn dep_contains() {
        let d = Dep::try_new("a/b").unwrap();
        let target_dep = Dependency::package("a/b", Default::default()).unwrap();
        let dep = Dependency::package("!u? ( a/b )", Default::default()).unwrap();
        let dep_ref = dep.to_ref();

        // Dependency objects
        assert!(dep.contains(&dep), "{dep} doesn't contain itself");
        assert!(dep_ref.contains(&dep), "{dep_ref} doesn't contain itself");
        assert!(dep.contains(&target_dep), "{dep} doesn't contain {target_dep}");
        assert!(dep_ref.contains(&target_dep), "{dep_ref} doesn't contain {target_dep}");

        // contains string types
        let s = "a/b".to_string();
        assert!(dep.contains(s.as_str()), "{dep} doesn't contain {s}");
        assert!(dep_ref.contains(s.as_str()), "{dep_ref} doesn't contain {s}");
        assert!(dep.contains(s.clone()), "{dep} doesn't contain {s}");
        assert!(dep_ref.contains(s.clone()), "{dep_ref} doesn't contain {s}");

        // Dep objects
        assert!(dep.contains(&d), "{dep} doesn't contain {d}");
        assert!(dep_ref.contains(&d), "{dep_ref} doesn't contain {d}");

        // UseDep objects
        let use_dep = UseDep::try_new("!u?").unwrap();
        assert!(dep.contains(&use_dep), "{dep} doesn't contain {use_dep}");
        assert!(dep_ref.contains(&use_dep), "{dep_ref} doesn't contain {use_dep}");

        // string-based Dependency
        let dep = Dependency::required_use("!u? ( a )").unwrap();
        let dep_ref = dep.to_ref();
        let s = "a".to_string();
        assert!(dep.contains(s.as_str()), "{dep} doesn't contain {s}");
        assert!(dep_ref.contains(s.as_str()), "{dep_ref} doesn't contain {s}");
        assert!(dep.contains(s.clone()), "{dep} doesn't contain {s}");
        assert!(dep_ref.contains(s.clone()), "{dep_ref} doesn't contain {s}");
    }

    #[test]
    fn dep_to_ref_and_into_owned() {
        for (s, len) in [
            ("a", 1),
            ("!a", 1),
            ("( a b )", 2),
            ("( a !b )", 2),
            ("|| ( a b )", 2),
            ("^^ ( a b )", 2),
            ("?? ( a b )", 2),
            ("u? ( a b )", 2),
            ("!u? ( a b )", 2),
        ] {
            let dep = Dependency::required_use(s).unwrap();
            assert!(!dep.is_empty());
            assert_eq!(dep.len(), len);
            let dep_ref = dep.to_ref();
            assert_eq!(&dep, &dep_ref);
            assert_eq!(&dep_ref, &dep);
            let dep_owned = dep_ref.into_owned();
            assert_eq!(&dep, &dep_owned);
        }
    }

    #[test]
    fn dep_iter() {
        for (s, expected) in [
            ("( ( a ) )", vec!["( a )"]),
            ("a", vec![]),
            ("!a", vec![]),
            ("( a b )", vec!["a", "b"]),
            ("( a !b )", vec!["a", "!b"]),
            ("|| ( a b )", vec!["a", "b"]),
            ("^^ ( a b )", vec!["a", "b"]),
            ("?? ( a b )", vec!["a", "b"]),
            ("u? ( a b )", vec!["a", "b"]),
            ("u1? ( a !u2? ( b ) )", vec!["a", "!u2? ( b )"]),
        ] {
            let dep = Dependency::required_use(s).unwrap();
            // borrowed
            assert_ordered_eq!(dep.iter().map(|x| x.to_string()), expected.iter().copied(), s);
            // owned
            assert_ordered_eq!(
                dep.clone().into_iter().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // borrowed and reversed
            assert_ordered_eq!(
                dep.iter().rev().map(|x| x.to_string()),
                expected.iter().rev().copied(),
                s
            );
            // owned and reversed
            assert_ordered_eq!(
                dep.clone().into_iter().rev().map(|x| x.to_string()),
                expected.iter().rev().copied(),
                s
            );
        }
    }

    #[test]
    fn dep_iter_flatten() {
        for (s, expected) in [
            ("( ( a ) )", vec!["a"]),
            ("a", vec!["a"]),
            ("!a", vec!["a"]),
            ("( a b )", vec!["a", "b"]),
            ("( a !b )", vec!["a", "b"]),
            ("|| ( a b )", vec!["a", "b"]),
            ("^^ ( a b )", vec!["a", "b"]),
            ("?? ( a b )", vec!["a", "b"]),
            ("u? ( a b )", vec!["a", "b"]),
            ("u1? ( a !u2? ( b ) )", vec!["a", "b"]),
        ] {
            let dep = Dependency::required_use(s).unwrap();
            // borrowed
            assert_ordered_eq!(
                dep.iter_flatten().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // owned
            assert_ordered_eq!(
                dep.clone().into_iter_flatten().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // borrowed and reversed
            assert_ordered_eq!(
                dep.iter_flatten().rev().map(|x| x.to_string()),
                expected.iter().rev().copied(),
                s
            );
            // owned and reversed
            assert_ordered_eq!(
                dep.clone().into_iter_flatten().rev().map(|x| x.to_string()),
                expected.iter().rev().copied(),
                s
            );
        }
    }

    #[test]
    fn dep_iter_recursive() {
        for (s, expected) in [
            ("( ( a ) )", vec!["( ( a ) )", "( a )", "a"]),
            ("a", vec!["a"]),
            ("!a", vec!["!a"]),
            ("( a b )", vec!["( a b )", "a", "b"]),
            ("( a !b )", vec!["( a !b )", "a", "!b"]),
            ("|| ( a b )", vec!["|| ( a b )", "a", "b"]),
            ("^^ ( a b )", vec!["^^ ( a b )", "a", "b"]),
            ("?? ( a b )", vec!["?? ( a b )", "a", "b"]),
            ("u? ( a b )", vec!["u? ( a b )", "a", "b"]),
            ("u1? ( a !u2? ( b ) )", vec!["u1? ( a !u2? ( b ) )", "a", "!u2? ( b )", "b"]),
        ] {
            let dep = Dependency::required_use(s).unwrap();
            // borrowed
            assert_ordered_eq!(
                dep.iter_recursive().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // owned
            assert_ordered_eq!(
                dep.into_iter_recursive().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
        }
    }

    #[test]
    fn dep_iter_conditionals() {
        for (s, expected) in [
            ("u? ( a )", vec!["u?"]),
            ("a", vec![]),
            ("!a", vec![]),
            ("( a b )", vec![]),
            ("( a !b )", vec![]),
            ("|| ( a b )", vec![]),
            ("^^ ( a b )", vec![]),
            ("?? ( a b )", vec![]),
            ("u? ( a b )", vec!["u?"]),
            ("u1? ( a !u2? ( b ) )", vec!["u1?", "!u2?"]),
        ] {
            let dep = Dependency::required_use(s).unwrap();
            // borrowed
            assert_ordered_eq!(
                dep.iter_conditionals().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // owned
            assert_ordered_eq!(
                dep.into_iter_conditionals().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
        }
    }

    #[test]
    fn dep_sort() {
        // dependencies
        for (s, expected) in [
            ("a/b", "a/b"),
            ("( c/d a/b )", "( a/b c/d )"),
            ("|| ( c/d a/b )", "|| ( c/d a/b )"),
            ("u? ( c/d a/b )", "u? ( a/b c/d )"),
            ("!u? ( c/d a/b )", "!u? ( a/b c/d )"),
        ] {
            let mut dep = Dependency::package(s, Default::default()).unwrap();
            dep.sort();
            assert_eq!(dep.to_string(), expected);
        }

        // REQUIRED_USE
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
            let mut dep = Dependency::required_use(s).unwrap();
            dep.sort();
            assert_eq!(dep.to_string(), expected);
        }
    }

    #[test]
    fn dep_set_variants() {
        // DependencySet<Dep>
        DependencySet::package("a/b c/d", Default::default()).unwrap();
        // DependencySet<String>
        DependencySet::license("a b").unwrap();
        DependencySet::properties("a b").unwrap();
        DependencySet::required_use("a b").unwrap();
        DependencySet::restrict("a b").unwrap();
        // DependencySet<Uri>
        DependencySet::src_uri("https://uri1 https://uri2").unwrap();
    }

    #[test]
    fn dep_set_contains() {
        let d = Dep::try_new("a/b").unwrap();
        let target_dep = Dependency::package("c/d", Default::default()).unwrap();
        let dep_set = DependencySet::package("a/b !u? ( c/d )", Default::default()).unwrap();
        let dep_set_ref = dep_set.to_ref();

        // Dependency objects
        assert!(dep_set.contains(&target_dep), "{dep_set} doesn't contain {target_dep}");
        assert!(dep_set_ref.contains(&target_dep), "{dep_set_ref} doesn't contain {target_dep}");

        // contains string types
        let s = "c/d".to_string();
        assert!(dep_set.contains(s.as_str()), "{dep_set} doesn't contain {s}");
        assert!(dep_set_ref.contains(s.as_str()), "{dep_set_ref} doesn't contain {s}");
        assert!(dep_set.contains(s.clone()), "{dep_set} doesn't contain {s}");
        assert!(dep_set_ref.contains(s.clone()), "{dep_set_ref} doesn't contain {s}");

        // Dep objects
        assert!(dep_set.contains(&d), "{dep_set} doesn't contain {d}");
        assert!(dep_set_ref.contains(&d), "{dep_set_ref} doesn't contain {d}");

        // UseDep objects
        let use_dep = UseDep::try_new("!u?").unwrap();
        assert!(dep_set.contains(&use_dep), "{dep_set} doesn't contain {use_dep}");
        assert!(dep_set_ref.contains(&use_dep), "{dep_set_ref} doesn't contain {use_dep}");

        // string-based DependencySet
        let dep_set = DependencySet::required_use("a !u? ( b )").unwrap();
        let dep_set_ref = dep_set.to_ref();
        let s = "b".to_string();
        assert!(dep_set.contains(s.as_str()), "{dep_set} doesn't contain {s}");
        assert!(dep_set_ref.contains(s.as_str()), "{dep_set_ref} doesn't contain {s}");
        assert!(dep_set.contains(s.clone()), "{dep_set} doesn't contain {s}");
        assert!(dep_set_ref.contains(s.clone()), "{dep_set_ref} doesn't contain {s}");
    }

    #[test]
    fn dep_set_to_ref_and_into_owned() {
        for (s, len) in [
            ("", 0),
            ("a b", 2),
            ("!a b", 2),
            ("( a b ) c", 2),
            ("( a !b ) c", 2),
            ("|| ( a b ) c", 2),
            ("^^ ( a b ) c", 2),
            ("?? ( a b ) c", 2),
            ("u? ( a b ) c", 2),
            ("!u? ( a b ) c", 2),
        ] {
            let dep_set = DependencySet::required_use(s).unwrap();
            assert_eq!(dep_set.is_empty(), s.is_empty());
            assert_eq!(dep_set.len(), len);
            let dep_set_ref = dep_set.to_ref();
            assert_eq!(&dep_set, &dep_set_ref);
            assert_eq!(&dep_set_ref, &dep_set);
            let dep_set_owned = dep_set_ref.into_owned();
            assert_eq!(&dep_set, &dep_set_owned);
        }
    }

    #[test]
    fn dep_set_iter() {
        for (s, expected) in [
            ("( a ) b", vec!["( a )", "b"]),
            ("a", vec!["a"]),
            ("!a", vec!["!a"]),
            ("( a b ) c", vec!["( a b )", "c"]),
            ("( a !b ) c", vec!["( a !b )", "c"]),
            ("|| ( a b ) c", vec!["|| ( a b )", "c"]),
            ("^^ ( a b ) c", vec!["^^ ( a b )", "c"]),
            ("?? ( a b ) c", vec!["?? ( a b )", "c"]),
            ("u? ( a b ) c", vec!["u? ( a b )", "c"]),
            ("u1? ( a !u2? ( b ) ) c", vec!["u1? ( a !u2? ( b ) )", "c"]),
        ] {
            let dep_set = DependencySet::required_use(s).unwrap();
            // borrowed
            assert_ordered_eq!(dep_set.iter().map(|x| x.to_string()), expected.iter().copied(), s);
            // owned
            assert_ordered_eq!(
                dep_set.clone().into_iter().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // borrowed and reversed
            assert_ordered_eq!(
                dep_set.iter().rev().map(|x| x.to_string()),
                expected.iter().rev().copied(),
                s
            );
            // owned and reversed
            assert_ordered_eq!(
                dep_set.clone().into_iter().rev().map(|x| x.to_string()),
                expected.iter().rev().copied(),
                s
            );
        }
    }

    #[test]
    fn dep_set_iter_flatten() {
        for (s, expected) in [
            ("( a ) b", vec!["a", "b"]),
            ("a", vec!["a"]),
            ("!a", vec!["a"]),
            ("( a b ) c", vec!["a", "b", "c"]),
            ("( a !b ) c", vec!["a", "b", "c"]),
            ("|| ( a b ) c", vec!["a", "b", "c"]),
            ("^^ ( a b ) c", vec!["a", "b", "c"]),
            ("?? ( a b ) c", vec!["a", "b", "c"]),
            ("u? ( a b ) c", vec!["a", "b", "c"]),
            ("u1? ( a !u2? ( b ) ) c", vec!["a", "b", "c"]),
        ] {
            let dep_set = DependencySet::required_use(s).unwrap();
            // borrowed
            assert_ordered_eq!(
                dep_set.iter_flatten().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // owned
            assert_ordered_eq!(
                dep_set.clone().into_iter_flatten().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // borrowed and reversed
            assert_ordered_eq!(
                dep_set.iter_flatten().rev().map(|x| x.to_string()),
                expected.iter().rev().copied(),
                s
            );
            // owned and reversed
            assert_ordered_eq!(
                dep_set
                    .clone()
                    .into_iter_flatten()
                    .rev()
                    .map(|x| x.to_string()),
                expected.iter().rev().copied(),
                s
            );
        }
    }

    #[test]
    fn dep_set_iter_recursive() {
        for (s, expected) in [
            ("( a ) b", vec!["( a )", "a", "b"]),
            ("a", vec!["a"]),
            ("!a", vec!["!a"]),
            ("( a b ) c", vec!["( a b )", "a", "b", "c"]),
            ("( a !b ) c", vec!["( a !b )", "a", "!b", "c"]),
            ("|| ( a b ) c", vec!["|| ( a b )", "a", "b", "c"]),
            ("^^ ( a b ) c", vec!["^^ ( a b )", "a", "b", "c"]),
            ("?? ( a b ) c", vec!["?? ( a b )", "a", "b", "c"]),
            ("u? ( a b ) c", vec!["u? ( a b )", "a", "b", "c"]),
            ("u1? ( a !u2? ( b ) ) c", vec!["u1? ( a !u2? ( b ) )", "a", "!u2? ( b )", "b", "c"]),
        ] {
            let dep_set = DependencySet::required_use(s).unwrap();
            // borrowed
            assert_ordered_eq!(
                dep_set.iter_recursive().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // owned
            assert_ordered_eq!(
                dep_set.into_iter_recursive().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
        }
    }

    #[test]
    fn dep_set_iter_conditionals() {
        for (s, expected) in [
            ("u? ( a ) b", vec!["u?"]),
            ("a", vec![]),
            ("!a", vec![]),
            ("( a b ) c", vec![]),
            ("( a !b ) c", vec![]),
            ("|| ( a b ) c", vec![]),
            ("^^ ( a b ) c", vec![]),
            ("?? ( a b ) c", vec![]),
            ("u? ( a b ) c", vec!["u?"]),
            ("u1? ( a !u2? ( b ) ) c", vec!["u1?", "!u2?"]),
        ] {
            let dep_set = DependencySet::required_use(s).unwrap();
            // borrowed
            assert_ordered_eq!(
                dep_set.iter_conditionals().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
            // owned
            assert_ordered_eq!(
                dep_set.into_iter_conditionals().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
        }
    }

    #[test]
    fn dep_set_sort() {
        // dependencies
        for (s, expected) in [
            ("c/d a/b", "a/b c/d"),
            ("( c/d a/b ) z/z", "z/z ( c/d a/b )"),
            ("|| ( c/d a/b ) z/z", "z/z || ( c/d a/b )"),
            ("u? ( c/d a/b ) z/z", "z/z u? ( c/d a/b )"),
            ("!u? ( c/d a/b ) z/z", "z/z !u? ( c/d a/b )"),
        ] {
            let mut set = DependencySet::package(s, Default::default()).unwrap();
            set.sort();
            assert_eq!(set.to_string(), expected);
        }

        // REQUIRED_USE
        for (s, expected) in [
            ("b a", "a b"),
            ("b !a", "b !a"),
            ("( b a ) z", "z ( b a )"),
            ("( b !a ) z", "z ( b !a )"),
            ("|| ( b a ) z", "z || ( b a )"),
            ("^^ ( b a ) z", "z ^^ ( b a )"),
            ("?? ( b a ) z", "z ?? ( b a )"),
            ("u? ( b a ) z", "z u? ( b a )"),
            ("!u? ( b a ) z", "z !u? ( b a )"),
        ] {
            let mut set = DependencySet::required_use(s).unwrap();
            set.sort();
            assert_eq!(set.to_string(), expected);
        }
    }

    #[test]
    fn dep_set_sort_recursive() {
        // dependencies
        for (s, expected) in [
            ("c/d a/b", "a/b c/d"),
            ("( c/d a/b ) z/z", "z/z ( a/b c/d )"),
            ("|| ( c/d a/b ) z/z", "z/z || ( c/d a/b )"),
            ("u? ( c/d a/b ) z/z", "z/z u? ( a/b c/d )"),
            ("!u? ( c/d a/b ) z/z", "z/z !u? ( a/b c/d )"),
        ] {
            let mut set = DependencySet::package(s, Default::default()).unwrap();
            set.sort_recursive();
            assert_eq!(set.to_string(), expected);
        }

        // REQUIRED_USE
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
            let mut set = DependencySet::required_use(s).unwrap();
            set.sort_recursive();
            assert_eq!(set.to_string(), expected);
        }
    }
}
