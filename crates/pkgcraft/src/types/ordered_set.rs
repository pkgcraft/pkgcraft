// TODO: This type can possibly be dropped if/when indexmap upstream implements an order-aware
// alternative type or changes IndexSet.
//
// See the following issues for more info:
// https://github.com/bluss/indexmap/issues/135
// https://github.com/bluss/indexmap/issues/153

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

use indexmap::IndexSet;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{make_set_traits, Ordered};

/// Ordered set that implements Ord and Hash.
#[derive(Debug, Clone)]
pub struct OrderedSet<T: Ordered>(IndexSet<T>);

impl<T: Ordered> Default for OrderedSet<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Ordered> From<IndexSet<T>> for OrderedSet<T> {
    fn from(value: IndexSet<T>) -> Self {
        Self(value)
    }
}

impl<T: Ordered> OrderedSet<T> {
    /// Construct a new, empty OrderedSet<T>.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return true if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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

impl<'de, T> Deserialize<'de> for OrderedSet<T>
where
    T: Ordered + Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        IndexSet::deserialize(deserializer).map(OrderedSet)
    }
}

impl<T> Serialize for OrderedSet<T>
where
    T: Serialize + Ordered,
{
    fn serialize<Se>(&self, serializer: Se) -> Result<Se::Ok, Se::Error>
    where
        Se: Serializer,
    {
        serializer.collect_seq(self)
    }
}

make_set_traits!(OrderedSet<T>);

#[cfg(test)]
mod tests {
    use crate::test::assert_ordered_eq;

    use super::*;

    #[test]
    fn eq_and_ord() {
        let s1 = OrderedSet::<&str>::new();
        let s2 = OrderedSet::<&str>::new();
        assert!(s1 >= s2);
        assert!(s1 <= s2);
        assert!(s1 == s2);

        let s1 = OrderedSet::from(["a"]);
        let s2 = OrderedSet::from(["a"]);
        assert!(s1 >= s2);
        assert!(s1 <= s2);
        assert!(s1 == s2);

        let s1 = OrderedSet::from(["a"]);
        let s2 = OrderedSet::from(["b"]);
        assert!(s1 < s2);

        let s1 = OrderedSet::from(["a", "b", "d"]);
        let s2 = OrderedSet::from(["a", "b", "c"]);
        assert!(s1 > s2);
    }

    #[test]
    fn hash() {
        // different elements
        let s1 = OrderedSet::from(["a"]);
        let s2 = OrderedSet::from(["b"]);
        assert_ne!(&s1, &s2);
        assert_eq!(OrderedSet::from([s1, s2]).len(), 2);

        // different ordering
        let s1 = OrderedSet::from(["a", "b"]);
        let s2 = OrderedSet::from(["b", "a"]);
        assert_ne!(&s1, &s2);
        assert_eq!(OrderedSet::from([s1, s2]).len(), 2);

        // similar ordering
        let s1 = OrderedSet::from(["a", "b"]);
        let s2 = OrderedSet::from(["a", "b"]);
        assert_eq!(&s1, &s2);
        assert_eq!(OrderedSet::from([s1, s2]).len(), 1);

        // matching elements
        let s1 = OrderedSet::from(["a", "b", "a"]);
        let s2 = OrderedSet::from(["a", "b", "b"]);
        assert_eq!(&s1, &s2);
        assert_eq!(OrderedSet::from([s1, s2]).len(), 1);
    }

    #[test]
    fn deref() {
        let mut set = OrderedSet::new();
        set.insert("a");
        assert!(set.contains("a"));
    }

    #[test]
    fn into_iter() {
        let items = ["a", "b", "c"];
        let set = OrderedSet::from(items);
        assert_ordered_eq!((&set).into_iter().copied(), items);
        assert_ordered_eq!(set.into_iter(), items);
    }

    #[test]
    fn and() {
        let s1 = OrderedSet::from(["a"]);
        let s2 = OrderedSet::from(["b"]);
        let s3 = &s1 & &s2;
        assert!(s3.is_empty());

        let s1 = OrderedSet::from(["a", "c"]);
        let s2 = OrderedSet::from(["b", "c"]);
        let s3 = &s1 & &s2;
        assert_ordered_eq!(&s3, &["c"]);

        let mut s1 = OrderedSet::from(["a", "c"]);
        let s2 = OrderedSet::from(["b", "c"]);
        s1 &= &s2;
        assert_ordered_eq!(&s1, &["c"]);
        s1 &= s2;
        assert_ordered_eq!(&s1, &["c"]);
    }

    #[test]
    fn or() {
        let s1 = OrderedSet::from(["a"]);
        let s2 = OrderedSet::from(["b"]);
        let s3 = &s1 | &s2;
        assert_ordered_eq!(&s3, &["a", "b"]);

        let s1 = OrderedSet::from(["a", "c"]);
        let s2 = OrderedSet::from(["b", "c"]);
        let s3 = &s1 | &s2;
        assert_ordered_eq!(&s3, &["a", "c", "b"]);

        let mut s1 = OrderedSet::from(["a", "c"]);
        let s2 = OrderedSet::from(["b", "c"]);
        s1 |= &s2;
        assert_ordered_eq!(&s1, &["a", "c", "b"]);
        s1 |= s2;
        assert_ordered_eq!(&s1, &["a", "c", "b"]);
    }

    #[test]
    fn xor() {
        let s1 = OrderedSet::from(["a"]);
        let s2 = OrderedSet::from(["b"]);
        let s3 = &s1 ^ &s2;
        assert_ordered_eq!(&s3, &["a", "b"]);

        let s1 = OrderedSet::from(["a", "c"]);
        let s2 = OrderedSet::from(["b", "c"]);
        let s3 = &s1 ^ &s2;
        assert_ordered_eq!(&s3, &["a", "b"]);

        let mut s1 = OrderedSet::from(["a", "c"]);
        let s2 = OrderedSet::from(["b", "c"]);
        s1 ^= &s2;
        assert_ordered_eq!(&s1, &["a", "b"]);
        s1 ^= s2;
        assert_ordered_eq!(&s1, &["a", "c"]);
    }

    #[test]
    fn sub() {
        let s1 = OrderedSet::from(["a"]);
        let s2 = OrderedSet::from(["b"]);
        let s3 = &s1 - &s2;
        assert_ordered_eq!(&s3, &["a"]);

        let s1 = OrderedSet::from(["a", "c"]);
        let s2 = OrderedSet::from(["b", "c"]);
        let s3 = &s1 - &s2;
        assert_ordered_eq!(&s3, &["a"]);

        let mut s1 = OrderedSet::from(["a", "c"]);
        let s2 = OrderedSet::from(["b", "c"]);
        s1 -= &s2;
        assert_ordered_eq!(&s1, &["a"]);
        s1 -= s2;
        assert_ordered_eq!(&s1, &["a"]);
    }
}
