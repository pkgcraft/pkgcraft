use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

use ordermap::OrderMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Ordered, OrderedSet};

/// Wrapper for OrderMap that provides additional FromIterator implemetations.
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Clone)]
pub struct OrderedMap<K: Ordered, V: Ordered>(OrderMap<K, V>);

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

    pub fn into_values(self) -> ordermap::map::IntoValues<K, V> {
        self.0.into_values()
    }
}

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
    type IntoIter = ordermap::map::Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<K: Ordered, V: Ordered> IntoIterator for OrderedMap<K, V> {
    type Item = (K, V);
    type IntoIter = ordermap::map::IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Ordered, V: Ordered> Deref for OrderedMap<K, V> {
    type Target = OrderMap<K, V>;

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
        OrderMap::deserialize(deserializer).map(OrderedMap)
    }
}

impl<K, V> Serialize for OrderedMap<K, V>
where
    K: Ordered + Serialize,
    V: Ordered + Serialize,
{
    fn serialize<Se>(&self, serializer: Se) -> Result<Se::Ok, Se::Error>
    where
        Se: Serializer,
    {
        OrderMap::serialize(self, serializer)
    }
}

#[cfg(test)]
mod tests {
    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn from_array() {
        let map = OrderedMap::from([("a", 1), ("b", 2)]);
        assert_ordered_eq!(&map, [(&"a", &1), (&"b", &2)]);
        assert_ordered_eq!(map, [("a", 1), ("b", 2)]);
    }
}
