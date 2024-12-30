use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer};

use super::{Ordered, OrderedSet};

/// Ordered map that implements Ord and Hash.
#[derive(Debug, Clone)]
pub struct OrderedMap<K: Ordered, V: Ordered>(pub(crate) IndexMap<K, V>);

impl<K: Ordered, V: Ordered> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K: Ordered, V: Ordered> OrderedMap<K, V> {
    /// Construct a new, empty OrderedMap<K, V>.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_values(self) -> indexmap::map::IntoValues<K, V> {
        self.0.into_values()
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

impl<K: Ordered, V: Ordered> FromIterator<(K, V)> for OrderedMap<K, Vec<V>> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iterable: I) -> Self {
        let mut map = Self::new();
        for (k, v) in iterable {
            map.entry(k).or_default().push(v);
        }
        map
    }
}

impl<K1: Ordered, K2: Ordered, V: Ordered> FromIterator<(K1, (K2, V))>
    for OrderedMap<K1, OrderedMap<K2, V>>
{
    fn from_iter<I: IntoIterator<Item = (K1, (K2, V))>>(iterable: I) -> Self {
        let mut map = Self::new();
        for (k1, (k2, v)) in iterable {
            map.entry(k1).or_default().insert(k2, v);
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
        IndexMap::deserialize(deserializer).map(OrderedMap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash() {
        // different elements
        let m1 = OrderedMap::from([("a", 1)]);
        let m2 = OrderedMap::from([("b", 2)]);
        assert_ne!(&m1, &m2);
        assert_eq!(OrderedSet::from([m1, m2]).len(), 2);

        // different ordering
        let m1 = OrderedMap::from([("a", 1), ("b", 2)]);
        let m2 = OrderedMap::from([("b", 2), ("a", 1)]);
        assert_ne!(&m1, &m2);
        assert_eq!(OrderedSet::from([m1, m2]).len(), 2);

        // similar ordering
        let m1 = OrderedMap::from([("a", 1), ("b", 2)]);
        let m2 = OrderedMap::from([("a", 1), ("b", 2)]);
        assert_eq!(&m1, &m2);
        assert_eq!(OrderedSet::from([m1, m2]).len(), 1);

        // matching elements
        let m1 = OrderedMap::from([("a", 1), ("b", 2), ("a", 1)]);
        let m2 = OrderedMap::from([("a", 1), ("b", 2), ("b", 2)]);
        assert_eq!(&m1, &m2);
        assert_eq!(OrderedSet::from([m1, m2]).len(), 1);
    }
}
