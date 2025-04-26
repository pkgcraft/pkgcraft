use std::ops::{Deref, DerefMut};

use ordermap::OrderSet;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Ordered, make_set_traits};

/// Wrapper for OrderSet that provides additional FromIterator implemetations.
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Clone)]
pub struct OrderedSet<T: Ordered>(OrderSet<T>);

impl<T: Ordered> Default for OrderedSet<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Ordered> From<OrderSet<T>> for OrderedSet<T> {
    fn from(value: OrderSet<T>) -> Self {
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
    type IntoIter = ordermap::set::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: Ordered> IntoIterator for OrderedSet<T> {
    type Item = T;
    type IntoIter = ordermap::set::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: Ordered> Deref for OrderedSet<T> {
    type Target = OrderSet<T>;

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
        OrderSet::deserialize(deserializer).map(OrderedSet)
    }
}

impl<T> Serialize for OrderedSet<T>
where
    T: Ordered + Serialize,
{
    fn serialize<Se>(&self, serializer: Se) -> Result<Se::Ok, Se::Error>
    where
        Se: Serializer,
    {
        OrderSet::serialize(self, serializer)
    }
}

make_set_traits!(OrderedSet<T>);

#[cfg(test)]
mod tests {
    use crate::test::assert_ordered_eq;

    use super::*;

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

    #[test]
    fn serde() {
        let set = OrderedSet::from(["a", "b"]);
        let s = serde_json::to_string(&set).unwrap();
        let obj: OrderedSet<_> = serde_json::from_str(&s).unwrap();
        assert_eq!(set, obj);
    }
}
