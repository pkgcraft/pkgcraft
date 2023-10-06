// TODO: This type can possibly be dropped if/when indexmap upstream implements an order-aware
// alternative type or changes IndexSet.
//
// See the following issues for more info:
// https://github.com/bluss/indexmap/issues/135
// https://github.com/bluss/indexmap/issues/153

use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use serde::{Deserialize, Deserializer};

pub trait Ordered: Debug + PartialEq + Eq + PartialOrd + Ord + Clone + Hash {}
impl<T> Ordered for T where T: Debug + PartialEq + Eq + PartialOrd + Ord + Clone + Hash {}

#[derive(Debug, Clone)]
pub struct OrderedSet<T: Ordered>(pub(crate) IndexSet<T>);

impl<T: Ordered> Default for OrderedSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ordered> OrderedSet<T> {
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
pub struct SortedSet<T: Ordered>(pub(crate) IndexSet<T>);

impl<T: Ordered> Default for SortedSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ordered> SortedSet<T> {
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

impl<T: Ordered> PartialOrd for SortedSet<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ordered> PartialEq for SortedSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
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
