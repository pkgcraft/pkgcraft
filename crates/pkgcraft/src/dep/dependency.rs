use std::fmt;

use itertools::Itertools;

use crate::eapi::Eapi;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::{Contains, IntoOwned, ToRef};
use crate::types::{Ordered, OrderedSet, SortedSet};

use super::*;

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
        match self {
            Self::Enabled(_) | Self::Disabled(_) => None,
            Self::AllOf(vals) => vals.get_index(index).map(AsRef::as_ref),
            Self::AnyOf(vals) => vals.get_index(index).map(AsRef::as_ref),
            Self::ExactlyOneOf(vals) => vals.get_index(index).map(AsRef::as_ref),
            Self::AtMostOneOf(vals) => vals.get_index(index).map(AsRef::as_ref),
            Self::Conditional(_, vals) => vals.get_index(index).map(AsRef::as_ref),
        }
    }

    /// Return true if a Dependency is empty, otherwise false.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Enabled(_) | Self::Disabled(_) => false,
            Self::AllOf(vals) => vals.is_empty(),
            Self::AnyOf(vals) => vals.is_empty(),
            Self::ExactlyOneOf(vals) => vals.is_empty(),
            Self::AtMostOneOf(vals) => vals.is_empty(),
            Self::Conditional(_, vals) => vals.is_empty(),
        }
    }

    /// Return the number of `Dependency` objects a `Dependency` contains.
    pub fn len(&self) -> usize {
        match self {
            Self::Enabled(_) => 1,
            Self::Disabled(_) => 1,
            Self::AllOf(vals) => vals.len(),
            Self::AnyOf(vals) => vals.len(),
            Self::ExactlyOneOf(vals) => vals.len(),
            Self::AtMostOneOf(vals) => vals.len(),
            Self::Conditional(_, vals) => vals.len(),
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
        match self {
            Self::Enabled(_) | Self::Disabled(_) => [].into_iter().collect(),
            Self::AllOf(vals) => vals.into_iter().map(|x| *x).collect(),
            Self::AnyOf(vals) => vals.into_iter().map(|x| *x).collect(),
            Self::ExactlyOneOf(vals) => vals.into_iter().map(|x| *x).collect(),
            Self::AtMostOneOf(vals) => vals.into_iter().map(|x| *x).collect(),
            Self::Conditional(_, vals) => vals.into_iter().map(|x| *x).collect(),
        }
    }
}

impl<'a, T: Ordered> Flatten for &'a Dependency<T> {
    type Item = &'a T;
    type IntoIterFlatten = IterFlatten<'a, T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        [self].into_iter().collect()
    }
}

impl<'a, T: Ordered> Recursive for &'a Dependency<T> {
    type Item = &'a Dependency<T>;
    type IntoIterRecursive = IterRecursive<'a, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        [self].into_iter().collect()
    }
}

impl<'a, T: Ordered> Conditionals for &'a Dependency<T> {
    type Item = &'a UseDep;
    type IntoIterConditionals = IterConditionals<'a, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        [self].into_iter().collect()
    }
}

impl<T: Ordered> Flatten for Dependency<T> {
    type Item = T;
    type IntoIterFlatten = IntoIterFlatten<T>;

    fn into_iter_flatten(self) -> Self::IntoIterFlatten {
        [self].into_iter().collect()
    }
}

impl<T: Ordered> Recursive for Dependency<T> {
    type Item = Dependency<T>;
    type IntoIterRecursive = IntoIterRecursive<T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        [self].into_iter().collect()
    }
}

impl<T: Ordered> Conditionals for Dependency<T> {
    type Item = UseDep;
    type IntoIterConditionals = IntoIterConditionals<T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        [self].into_iter().collect()
    }
}

impl<T: fmt::Display + Ordered> fmt::Display for Dependency<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Enabled(val) => write!(f, "{val}"),
            Self::Disabled(val) => write!(f, "!{val}"),
            Self::AllOf(vals) => write!(f, "( {} )", p!(vals)),
            Self::AnyOf(vals) => write!(f, "|| ( {} )", p!(vals)),
            Self::ExactlyOneOf(vals) => write!(f, "^^ ( {} )", p!(vals)),
            Self::AtMostOneOf(vals) => write!(f, "?? ( {} )", p!(vals)),
            Self::Conditional(u, vals) => write!(f, "{u} ( {} )", p!(vals)),
        }
    }
}

impl Restriction<&Dependency<Dep>> for BaseRestrict {
    fn matches(&self, val: &Dependency<Dep>) -> bool {
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
}
