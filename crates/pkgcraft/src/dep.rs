use std::borrow::Borrow;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Sub,
    SubAssign,
};

use indexmap::IndexSet;
use itertools::Itertools;

use crate::eapi::Eapi;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::{Contains, IntoOwned, ToRef};
use crate::types::{Deque, Ordered};

pub mod cpn;
pub use cpn::Cpn;
pub mod cpv;
pub use cpv::{Cpv, CpvOrDep};
mod dependency;
pub use dependency::Dependency;
mod dependency_iter;
pub use dependency_iter::*;
mod dependency_set;
pub use dependency_set::DependencySet;
pub mod parse;
pub mod pkg;
pub use pkg::{Blocker, Dep, DepField, Slot, SlotDep, SlotOperator};
pub mod uri;
pub use uri::Uri;
pub mod use_dep;
pub use use_dep::{UseDep, UseDepKind};
pub mod version;
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

/// Convert iterator into a space-separated string of values.
#[macro_export]
macro_rules! p {
    ($x:expr) => {
        $x.into_iter().map(|x| x.to_string()).join(" ")
    };
}
use p;

/// Recursively sort a set with sortable elements into an iterator.
// TODO: replace with in-place mutation when IndexSet supports IndexMut and/or iter_mut()
#[macro_export]
macro_rules! sort_set {
    ($vals:expr) => {
        itertools::sorted($vals.clone().into_iter().map(|mut d| {
            d.sort();
            d
        }))
    };
}
use sort_set;
