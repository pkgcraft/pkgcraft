use std::fmt;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Sub,
    SubAssign,
};

use itertools::Itertools;

use crate::eapi::Eapi;
use crate::restrict::{Restrict as BaseRestrict, Restriction};
use crate::traits::{Contains, IntoOwned, ToRef};
use crate::types::{Ordered, SortedSet};

use super::*;

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

    pub fn iter_conditional_flatten(&self) -> IterConditionalFlatten<T> {
        self.into_iter_conditional_flatten()
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

impl<T: fmt::Display + Ordered> fmt::Display for DependencySet<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

impl<'a, T: Ordered> BitAnd<&'a DependencySet<T>> for &'a DependencySet<T> {
    type Output = DependencySet<&'a T>;

    fn bitand(self, other: &'a DependencySet<T>) -> Self::Output {
        self.intersection(other).collect()
    }
}

impl<'a, T: Ordered> BitAnd<&'a DependencySet<T>> for DependencySet<&'a T> {
    type Output = DependencySet<&'a T>;

    fn bitand(self, other: &'a DependencySet<T>) -> Self::Output {
        self.intersection(&other.to_ref()).cloned().collect()
    }
}

impl<'a, T: Ordered> BitAnd<DependencySet<&'a T>> for &'a DependencySet<T> {
    type Output = DependencySet<&'a T>;

    fn bitand(self, other: DependencySet<&'a T>) -> Self::Output {
        self.to_ref().intersection(&other).cloned().collect()
    }
}

impl<T: Ordered> BitAndAssign<&DependencySet<T>> for DependencySet<T> {
    fn bitand_assign(&mut self, other: &DependencySet<T>) {
        self.retain(|x| other.contains(x))
    }
}

impl<T: Ordered> BitAndAssign<DependencySet<&T>> for DependencySet<T> {
    fn bitand_assign(&mut self, other: DependencySet<&T>) {
        self.retain(|x| other.contains(x))
    }
}

impl<T: Ordered> BitAndAssign<&DependencySet<T>> for DependencySet<&T> {
    fn bitand_assign(&mut self, other: &DependencySet<T>) {
        self.retain(|x| other.contains(x))
    }
}

impl<'a, T: Ordered> BitOr<&'a DependencySet<T>> for &'a DependencySet<T> {
    type Output = DependencySet<&'a T>;

    fn bitor(self, other: &'a DependencySet<T>) -> Self::Output {
        self.union(other).collect()
    }
}

impl<'a, T: Ordered> BitOr<&'a DependencySet<T>> for DependencySet<&'a T> {
    type Output = DependencySet<&'a T>;

    fn bitor(self, other: &'a DependencySet<T>) -> Self::Output {
        self.union(&other.to_ref()).cloned().collect()
    }
}

impl<'a, T: Ordered> BitOr<DependencySet<&'a T>> for &'a DependencySet<T> {
    type Output = DependencySet<&'a T>;

    fn bitor(self, other: DependencySet<&'a T>) -> Self::Output {
        self.to_ref().union(&other).cloned().collect()
    }
}

impl<T: Ordered> BitOrAssign<&DependencySet<T>> for DependencySet<T> {
    fn bitor_assign(&mut self, other: &DependencySet<T>) {
        self.extend(other.clone())
    }
}

impl<T: Ordered> BitOrAssign<DependencySet<&T>> for DependencySet<T> {
    fn bitor_assign(&mut self, other: DependencySet<&T>) {
        self.extend(other.into_owned())
    }
}

impl<'a, T: Ordered> BitOrAssign<&'a DependencySet<T>> for DependencySet<&'a T> {
    fn bitor_assign(&mut self, other: &'a DependencySet<T>) {
        self.extend(other.to_ref())
    }
}

impl<'a, T: Ordered> BitXor<&'a DependencySet<T>> for &'a DependencySet<T> {
    type Output = DependencySet<&'a T>;

    fn bitxor(self, other: &'a DependencySet<T>) -> Self::Output {
        self.symmetric_difference(other).collect()
    }
}

impl<'a, T: Ordered> BitXor<&'a DependencySet<T>> for DependencySet<&'a T> {
    type Output = DependencySet<&'a T>;

    fn bitxor(self, other: &'a DependencySet<T>) -> Self::Output {
        self.symmetric_difference(&other.to_ref())
            .cloned()
            .collect()
    }
}

impl<'a, T: Ordered> BitXor<DependencySet<&'a T>> for &'a DependencySet<T> {
    type Output = DependencySet<&'a T>;

    fn bitxor(self, other: DependencySet<&'a T>) -> Self::Output {
        self.to_ref()
            .symmetric_difference(&other)
            .cloned()
            .collect()
    }
}

impl<T: Ordered> BitXorAssign<&DependencySet<T>> for DependencySet<T> {
    fn bitxor_assign(&mut self, other: &DependencySet<T>) {
        self.0 = &self.0 ^ &other.0;
    }
}

impl<T: Ordered> BitXorAssign<DependencySet<&T>> for DependencySet<T> {
    fn bitxor_assign(&mut self, other: DependencySet<&T>) {
        self.0 = &self.0 ^ &other.into_owned().0;
    }
}

impl<'a, T: Ordered> BitXorAssign<&'a DependencySet<T>> for DependencySet<&'a T> {
    fn bitxor_assign(&mut self, other: &'a DependencySet<T>) {
        self.0 = &self.0 ^ &other.to_ref().0;
    }
}

impl<'a, T: Ordered> Sub<&'a DependencySet<T>> for &'a DependencySet<T> {
    type Output = DependencySet<&'a T>;

    fn sub(self, other: &'a DependencySet<T>) -> Self::Output {
        self.difference(other).collect()
    }
}

impl<'a, T: Ordered> Sub<&'a DependencySet<T>> for DependencySet<&'a T> {
    type Output = DependencySet<&'a T>;

    fn sub(self, other: &'a DependencySet<T>) -> Self::Output {
        self.difference(&other.to_ref()).cloned().collect()
    }
}

impl<'a, T: Ordered> Sub<DependencySet<&'a T>> for &'a DependencySet<T> {
    type Output = DependencySet<&'a T>;

    fn sub(self, other: DependencySet<&'a T>) -> Self::Output {
        self.to_ref().difference(&other).cloned().collect()
    }
}

impl<T: Ordered> SubAssign<&DependencySet<T>> for DependencySet<T> {
    fn sub_assign(&mut self, other: &DependencySet<T>) {
        self.retain(|x| !other.contains(x))
    }
}

impl<T: Ordered> SubAssign<DependencySet<&T>> for DependencySet<T> {
    fn sub_assign(&mut self, other: DependencySet<&T>) {
        self.retain(|x| !other.contains(x))
    }
}

impl<'a, T: Ordered> SubAssign<&'a DependencySet<T>> for DependencySet<&'a T> {
    fn sub_assign(&mut self, other: &'a DependencySet<T>) {
        self.retain(|x| !other.contains(x))
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
        self.0.iter().collect()
    }
}

impl<'a, T: Ordered> Recursive for &'a DependencySet<T> {
    type Item = &'a Dependency<T>;
    type IntoIterRecursive = IterRecursive<'a, T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        self.0.iter().collect()
    }
}

impl<'a, T: Ordered> Conditionals for &'a DependencySet<T> {
    type Item = &'a UseDep;
    type IntoIterConditionals = IterConditionals<'a, T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        self.0.iter().collect()
    }
}

impl<'a, T: Ordered> ConditionalFlatten for &'a DependencySet<T> {
    type Item = (Vec<&'a UseDep>, &'a T);
    type IntoIterConditionalFlatten = IterConditionalFlatten<'a, T>;

    fn into_iter_conditional_flatten(self) -> Self::IntoIterConditionalFlatten {
        self.0.iter().collect()
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
        self.0.into_iter().collect()
    }
}

impl<T: Ordered> Recursive for DependencySet<T> {
    type Item = Dependency<T>;
    type IntoIterRecursive = IntoIterRecursive<T>;

    fn into_iter_recursive(self) -> Self::IntoIterRecursive {
        self.0.into_iter().collect()
    }
}

impl<T: Ordered> Conditionals for DependencySet<T> {
    type Item = UseDep;
    type IntoIterConditionals = IntoIterConditionals<T>;

    fn into_iter_conditionals(self) -> Self::IntoIterConditionals {
        self.0.into_iter().collect()
    }
}

impl<T: Ordered> ConditionalFlatten for DependencySet<T> {
    type Item = (Vec<UseDep>, T);
    type IntoIterConditionalFlatten = IntoIterConditionalFlatten<T>;

    fn into_iter_conditional_flatten(self) -> Self::IntoIterConditionalFlatten {
        self.0.into_iter().collect()
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
    fn new() {
        let mut set = DependencySet::<Dep>::new();
        assert!(set.is_empty());
        let dep = Dependency::package("cat/pkg", Default::default()).unwrap();
        set.insert(dep);
        assert_eq!(set.to_string(), "cat/pkg");
    }

    #[test]
    fn from_iter() {
        let set1 = DependencySet::required_use("a b c").unwrap();

        // owned
        let set2: DependencySet<String> = set1.clone().into_iter().collect();
        assert_eq!(&set1, &set2);

        // borrowed
        let set2: DependencySet<&String> = set1.iter().collect();
        assert_eq!(&set1, &set2);
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
        assert!(
            dep_set_ref.contains(&target_dep),
            "{dep_set_ref} doesn't contain {target_dep}"
        );

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
            ("a", vec!["a"]),
            ("!a", vec!["!a"]),
            ("( a ) b", vec!["( a )", "b"]),
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
            assert_ordered_eq!(
                dep_set.iter().map(|x| x.to_string()),
                expected.iter().copied(),
                s
            );
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
            ("a", vec!["a"]),
            ("!a", vec!["a"]),
            ("( a ) b", vec!["a", "b"]),
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
            ("a", vec!["a"]),
            ("!a", vec!["!a"]),
            ("( a ) b", vec!["( a )", "a", "b"]),
            ("( a b ) c", vec!["( a b )", "a", "b", "c"]),
            ("( a !b ) c", vec!["( a !b )", "a", "!b", "c"]),
            ("|| ( a b ) c", vec!["|| ( a b )", "a", "b", "c"]),
            ("^^ ( a b ) c", vec!["^^ ( a b )", "a", "b", "c"]),
            ("?? ( a b ) c", vec!["?? ( a b )", "a", "b", "c"]),
            ("u? ( a b ) c", vec!["u? ( a b )", "a", "b", "c"]),
            (
                "u1? ( a !u2? ( b ) ) c",
                vec!["u1? ( a !u2? ( b ) )", "a", "!u2? ( b )", "b", "c"],
            ),
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
            ("a", vec![]),
            ("!a", vec![]),
            ("( a ) b", vec![]),
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
    fn dep_iter_conditional_flatten() {
        for (s, expected) in [
            ("a", vec![(vec![], "a")]),
            ("!a", vec![(vec![], "a")]),
            ("( a ) b", vec![(vec![], "a"), (vec![], "b")]),
            ("( a b ) c", vec![(vec![], "a"), (vec![], "b"), (vec![], "c")]),
            ("( a !b ) c", vec![(vec![], "a"), (vec![], "b"), (vec![], "c")]),
            ("|| ( a b ) c", vec![(vec![], "a"), (vec![], "b"), (vec![], "c")]),
            ("^^ ( a b ) c", vec![(vec![], "a"), (vec![], "b"), (vec![], "c")]),
            ("?? ( a b ) c", vec![(vec![], "a"), (vec![], "b"), (vec![], "c")]),
            ("u? ( a b ) c", vec![(vec!["u?"], "a"), (vec!["u?"], "b"), (vec![], "c")]),
            (
                "u1? ( a !u2? ( b ) ) c",
                vec![(vec!["u1?"], "a"), (vec!["u1?", "!u2?"], "b"), (vec![], "c")],
            ),
        ] {
            let dep_set = DependencySet::required_use(s).unwrap();

            // borrowed
            let test = dep_set.iter_conditional_flatten();
            for ((test_use, test_dep), (expected_use, expected_dep)) in
                test.zip(expected.iter())
            {
                assert_ordered_eq!(
                    test_use.iter().map(|x| x.to_string()),
                    expected_use.iter().map(|x| x.to_string()),
                    s
                );
                assert_eq!(test_dep.to_string(), expected_dep.to_string(), "{s}");
            }

            // owned
            let test = dep_set.into_iter_conditional_flatten();
            for ((test_use, test_dep), (expected_use, expected_dep)) in
                test.zip(expected.iter())
            {
                assert_ordered_eq!(
                    test_use.iter().map(|x| x.to_string()),
                    expected_use.iter().map(|x| x.to_string()),
                    s
                );
                assert_eq!(test_dep.to_string(), expected_dep.to_string(), "{s}");
            }
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

    #[test]
    fn set_ops() {
        let set1 = DependencySet::required_use("1 2").unwrap();
        let set2 = DependencySet::required_use("2 3").unwrap();
        let set3 = DependencySet::required_use("3 4").unwrap();
        let set4 = DependencySet::required_use("4 5").unwrap();

        // intersection
        let set = &set1 & &set2;
        assert_eq!(set.to_string(), "2");
        let set = &set1 & &set2 & &set3;
        assert!(set.is_empty());
        let set = &set1 & (&set2 & &set3);
        assert!(set.is_empty());
        // assign
        let mut set = set1.clone();
        set &= &set2;
        assert_eq!(set.to_string(), "2");
        set &= &set3 & &set4;
        assert!(set.is_empty());
        let mut set = &set1 & &set2;
        set &= &set2;
        assert_eq!(set.to_string(), "2");

        // union
        let set = &set1 | &set2;
        assert_eq!(set.to_string(), "1 2 3");
        let set = &set1 | &set2 | &set3;
        assert_eq!(set.to_string(), "1 2 3 4");
        let set = &set1 | (&set2 | &set3);
        assert_eq!(set.to_string(), "1 2 3 4");
        // assign
        let mut set = set1.clone();
        set |= &set2;
        assert_eq!(set.to_string(), "1 2 3");
        set |= &set3 | &set4;
        assert_eq!(set.to_string(), "1 2 3 4 5");
        let mut set = &set1 | &set2;
        set |= &set2;
        assert_eq!(set.to_string(), "1 2 3");

        // difference
        let set = &set1 - &set2;
        assert_eq!(set.to_string(), "1");
        let set = &set1 - &set2 - &set3;
        assert_eq!(set.to_string(), "1");
        let set = &set1 - (&set2 - &set3);
        assert_eq!(set.to_string(), "1");
        let set = &set1 - &set1;
        assert!(set.is_empty());
        // assign
        let mut set = set1.clone();
        set -= &set2;
        assert_eq!(set.to_string(), "1");
        set -= &set3 - &set4;
        assert_eq!(set.to_string(), "1");
        let mut set = &set1 - &set2;
        set -= &set1;
        assert!(set.is_empty());

        // exclusive or
        let set = &set1 ^ &set2;
        assert_eq!(set.to_string(), "1 3");
        let set = &set1 ^ &set2 ^ &set3;
        assert_eq!(set.to_string(), "1 4");
        let set = &set1 ^ (&set2 ^ &set3);
        assert_eq!(set.to_string(), "1 4");
        // assign
        let mut set = set1.clone();
        set ^= &set2;
        assert_eq!(set.to_string(), "1 3");
        set ^= &set3 ^ &set4;
        assert_eq!(set.to_string(), "1 5");
        let mut set = &set1 ^ &set2;
        set ^= &set1;
        assert_eq!(set.to_string(), "3 2");
    }
}
