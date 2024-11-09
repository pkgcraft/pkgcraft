use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Deserializer};

use super::Ordered;

/// IndexMap wrapper supporting custom FromIterator implementations.
#[derive(Debug, Clone)]
pub struct IndexMap<K: Ordered, V>(pub(crate) indexmap::IndexMap<K, V>);

impl<K: Ordered, V> Default for IndexMap<K, V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K: Ordered, V> IndexMap<K, V> {
    /// Construct a new, empty IndexMap<K, V>.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_values(self) -> indexmap::map::IntoValues<K, V> {
        self.0.into_values()
    }
}

impl<K, V1, V2> PartialEq<IndexMap<K, V2>> for IndexMap<K, V1>
where
    K: Ordered,
    V1: PartialEq<V2>,
{
    fn eq(&self, other: &IndexMap<K, V2>) -> bool {
        self.0.eq(&other.0)
    }
}

impl<K, V> Eq for IndexMap<K, V>
where
    K: Ordered,
    V: Eq,
{
}

impl<K: Ordered, V> FromIterator<(K, V)> for IndexMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<K: Ordered, V> FromIterator<(K, V)> for IndexMap<K, Vec<V>> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iterable: I) -> Self {
        let mut map = Self::new();
        for (k, v) in iterable {
            map.entry(k).or_default().push(v);
        }
        map
    }
}

impl<'a, K: Ordered, V> IntoIterator for &'a IndexMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = indexmap::map::Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<K: Ordered, V> IntoIterator for IndexMap<K, V> {
    type Item = (K, V);
    type IntoIter = indexmap::map::IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Ordered, V> Deref for IndexMap<K, V> {
    type Target = indexmap::IndexMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K: Ordered, V> DerefMut for IndexMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'de, K, V> Deserialize<'de> for IndexMap<K, V>
where
    K: Ordered + Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        indexmap::IndexMap::deserialize(deserializer).map(IndexMap)
    }
}
