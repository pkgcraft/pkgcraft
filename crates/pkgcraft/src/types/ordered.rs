// TODO: This type can possibly be dropped if/when indexmap upstream implements an order-aware
// alternative type or changes IndexSet.
//
// See the following issues for more info:
// https://github.com/bluss/indexmap/issues/135
// https://github.com/bluss/indexmap/issues/153

use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Sub, SubAssign,
};

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use serde::{Deserialize, Deserializer};

use crate::macros::partial_cmp_not_equal_opt;

pub trait Ordered: Debug + PartialEq + Eq + PartialOrd + Ord + Clone + Hash {}
impl<T> Ordered for T where T: Debug + PartialEq + Eq + PartialOrd + Ord + Clone + Hash {}

#[derive(Debug, Clone)]
pub struct OrderedSet<T: Ordered>(IndexSet<T>);

impl<T: Ordered> Default for OrderedSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ordered> OrderedSet<T> {
    /// Constructs a new, empty OrderedSet<T>.
    pub fn new() -> Self {
        Self(IndexSet::new())
    }
}

impl<T: Ordered> Hash for OrderedSet<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for e in &self.0 {
            e.hash(state);
        }
    }
}

impl<T: Ordered> Ord for OrderedSet<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.iter().cmp(other.0.iter())
    }
}

impl<T: Ordered> PartialOrd for OrderedSet<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ordered> PartialEq for OrderedSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<T: Ordered> Eq for OrderedSet<T> {}

impl<T: Ordered> FromIterator<T> for OrderedSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<T: Ordered, const N: usize> From<[T; N]> for OrderedSet<T> {
    fn from(arr: [T; N]) -> Self {
        Self::from_iter(arr)
    }
}

impl<'a, T: Ordered> IntoIterator for &'a OrderedSet<T> {
    type Item = &'a T;
    type IntoIter = indexmap::set::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: Ordered> IntoIterator for OrderedSet<T> {
    type Item = T;
    type IntoIter = indexmap::set::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: Ordered> Deref for OrderedSet<T> {
    type Target = IndexSet<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Ordered> DerefMut for OrderedSet<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'de, T: Ordered + Deserialize<'de>> Deserialize<'de> for OrderedSet<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vals: Vec<T> = Deserialize::deserialize(deserializer)?;
        Ok(vals.into_iter().collect())
    }
}

#[derive(Debug, Clone)]
pub struct SortedSet<T: Ordered>(IndexSet<T>);

impl<T: Ordered> Default for SortedSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ordered> SortedSet<T> {
    /// Constructs a new, empty SortedSet<T>.
    pub fn new() -> Self {
        Self(IndexSet::new())
    }
}

impl<T: Ordered> Hash for SortedSet<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for e in self.0.iter().sorted() {
            e.hash(state);
        }
    }
}

impl<T: Ordered> Ord for SortedSet<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.iter().sorted().cmp(other.0.iter().sorted())
    }
}

impl<T1, T2> PartialOrd<SortedSet<T1>> for SortedSet<T2>
where
    T1: Ordered,
    T2: Ordered + PartialOrd<T1>,
{
    fn partial_cmp(&self, other: &SortedSet<T1>) -> Option<Ordering> {
        let mut self_iter = self.iter().sorted();
        let mut other_iter = other.iter().sorted();
        loop {
            match (self_iter.next(), other_iter.next()) {
                (Some(v1), Some(v2)) => partial_cmp_not_equal_opt!(v1, v2),
                (Some(_), None) => return Some(Ordering::Greater),
                (None, Some(_)) => return Some(Ordering::Less),
                (None, None) => return Some(Ordering::Equal),
            }
        }
    }
}

impl<T1, T2> PartialEq<SortedSet<T1>> for SortedSet<T2>
where
    T1: Ordered,
    T2: Ordered + PartialOrd<T1>,
{
    fn eq(&self, other: &SortedSet<T1>) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl<T: Ordered> Eq for SortedSet<T> {}

impl<T: Ordered> FromIterator<T> for SortedSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<T: Ordered, const N: usize> From<[T; N]> for SortedSet<T> {
    fn from(arr: [T; N]) -> Self {
        Self::from_iter(arr)
    }
}

impl<'a, T: Ordered> IntoIterator for &'a SortedSet<T> {
    type Item = &'a T;
    type IntoIter = indexmap::set::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: Ordered> IntoIterator for SortedSet<T> {
    type Item = T;
    type IntoIter = indexmap::set::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: Ordered> Deref for SortedSet<T> {
    type Target = IndexSet<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Ordered> DerefMut for SortedSet<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'de, T: Ordered + Deserialize<'de>> Deserialize<'de> for SortedSet<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vals: Vec<T> = Deserialize::deserialize(deserializer)?;
        Ok(vals.into_iter().collect())
    }
}

macro_rules! make_set_traits {
    ($($x:ty),+) => {$(
        impl<T: Ordered> BitAnd<&Self> for $x {
            type Output = Self;

            fn bitand(mut self, other: &Self) -> Self::Output {
                self &= other;
                self
            }
        }

        impl<T: Ordered> BitAndAssign<&Self> for $x {
            fn bitand_assign(&mut self, other: &Self) {
                self.0 = &self.0 & &other.0;
            }
        }

        impl<T: Ordered> BitOr<&Self> for $x {
            type Output = Self;

            fn bitor(mut self, other: &Self) -> Self::Output {
                self |= other;
                self
            }
        }

        impl<T: Ordered> BitOrAssign<&Self> for $x {
            fn bitor_assign(&mut self, other: &Self) {
                self.0 = &self.0 | &other.0;
            }
        }

        impl<T: Ordered> BitXor<&Self> for $x {
            type Output = Self;

            fn bitxor(mut self, other: &Self) -> Self::Output {
                self ^= other;
                self
            }
        }

        impl<T: Ordered> BitXorAssign<&Self> for $x {
            fn bitxor_assign(&mut self, other: &Self) {
                self.0 = &self.0 ^ &other.0;
            }
        }

        impl<T: Ordered> Sub<&Self> for $x {
            type Output = Self;

            fn sub(mut self, other: &Self) -> Self::Output {
                self -= other;
                self
            }
        }

        impl<T: Ordered> SubAssign<&Self> for $x {
            fn sub_assign(&mut self, other: &Self) {
                self.0 = &self.0 - &other.0;
            }
        }
    )+};
}
use make_set_traits;
make_set_traits!(OrderedSet<T>, SortedSet<T>);

#[derive(Debug, Clone)]
pub struct OrderedMap<K: Ordered, V: Ordered>(pub(crate) IndexMap<K, V>);

impl<K: Ordered, V: Ordered> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ordered, V: Ordered> OrderedMap<K, V> {
    pub fn new() -> Self {
        Self(IndexMap::new())
    }
}

impl<K: Ordered, V: Ordered> Hash for OrderedMap<K, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for e in &self.0 {
            e.hash(state);
        }
    }
}

impl<K: Ordered, V: Ordered> Ord for OrderedMap<K, V> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.iter().cmp(other.0.iter())
    }
}

impl<K: Ordered, V: Ordered> PartialOrd for OrderedMap<K, V> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Ordered, V: Ordered> PartialEq for OrderedMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<K: Ordered, V: Ordered> Eq for OrderedMap<K, V> {}

impl<K: Ordered, V: Ordered> FromIterator<(K, V)> for OrderedMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<K: Ordered, V: Ordered> FromIterator<(K, V)> for OrderedMap<K, OrderedSet<V>> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iterable: I) -> Self {
        let mut map = Self::new();
        for (k, v) in iterable {
            map.entry(k).or_default().insert(v);
        }
        map
    }
}

impl<K: Ordered, V: Ordered, const N: usize> From<[(K, V); N]> for OrderedMap<K, V> {
    fn from(arr: [(K, V); N]) -> Self {
        Self::from_iter(arr)
    }
}

impl<'a, K: Ordered, V: Ordered> IntoIterator for &'a OrderedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = indexmap::map::Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<K: Ordered, V: Ordered> IntoIterator for OrderedMap<K, V> {
    type Item = (K, V);
    type IntoIter = indexmap::map::IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Ordered, V: Ordered> Deref for OrderedMap<K, V> {
    type Target = IndexMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K: Ordered, V: Ordered> DerefMut for OrderedMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'de, K, V> Deserialize<'de> for OrderedMap<K, V>
where
    K: Ordered + Deserialize<'de>,
    V: Ordered + Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vals: Vec<(K, V)> = Deserialize::deserialize(deserializer)?;
        Ok(vals.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::hash;

    use super::*;

    #[test]
    fn test_ordered_set() {
        // different elements
        let s1 = OrderedSet::from(["a"]);
        let s2 = OrderedSet::from(["b"]);
        assert_ne!(&s1, &s2);
        assert_ne!(hash(&s1), hash(&s2));

        // different ordering
        let s1 = OrderedSet::from(["a", "b"]);
        let s2 = OrderedSet::from(["b", "a"]);
        assert_ne!(&s1, &s2);
        assert_ne!(hash(&s1), hash(&s2));

        // similar ordering
        let s1 = OrderedSet::from(["a", "b"]);
        let s2 = OrderedSet::from(["a", "b"]);
        assert_eq!(&s1, &s2);
        assert_eq!(hash(&s1), hash(&s2));

        // matching elements
        let s1 = OrderedSet::from(["a", "b", "a"]);
        let s2 = OrderedSet::from(["a", "b", "b"]);
        assert_eq!(&s1, &s2);
        assert_eq!(hash(&s1), hash(&s2));
    }

    #[test]
    fn test_sorted_set() {
        // different elements
        let s1 = SortedSet::from(["a"]);
        let s2 = SortedSet::from(["b"]);
        assert_ne!(&s1, &s2);
        assert_ne!(hash(&s1), hash(&s2));

        // different ordering
        let s1 = SortedSet::from(["a", "b"]);
        let s2 = SortedSet::from(["b", "a"]);
        assert_eq!(&s1, &s2);
        assert_eq!(hash(&s1), hash(&s2));

        // similar ordering
        let s1 = SortedSet::from(["a", "b"]);
        let s2 = SortedSet::from(["a", "b"]);
        assert_eq!(&s1, &s2);
        assert_eq!(hash(&s1), hash(&s2));

        // matching elements
        let s1 = SortedSet::from(["a", "b", "a"]);
        let s2 = SortedSet::from(["a", "b", "b"]);
        assert_eq!(&s1, &s2);
        assert_eq!(hash(&s1), hash(&s2));
    }
}
