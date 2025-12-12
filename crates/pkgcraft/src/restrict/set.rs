use crate::types::{Ordered, OrderedSet};

use super::ordered::Restrict as OrderedRestrict;
use super::str::Restrict as StrRestrict;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OrderedSetRestrict<T: Ordered, R> {
    Empty,
    Contains(T),
    Disjoint(OrderedSet<T>),
    Equal(OrderedSet<T>),
    Subset(OrderedSet<T>),
    ProperSubset(OrderedSet<T>),
    Superset(OrderedSet<T>),
    ProperSuperset(OrderedSet<T>),
    Ordered(OrderedRestrict<R>),
}

macro_rules! make_set_restriction {
    ($(($container:ty, $element:ty, $restrict:ty)),+) => {$(
        impl crate::restrict::Restriction<&$container> for OrderedSetRestrict<$element, $restrict> {
            fn matches(&self, val: &$container) -> bool {
                match self {
                    Self::Empty => val.is_empty(),
                    Self::Contains(s) => val.contains(s),
                    Self::Disjoint(s) => val.is_disjoint(s),
                    Self::Equal(s) => val == s,
                    Self::Subset(s) => val.is_subset(s),
                    Self::ProperSubset(s) => val.is_subset(s) && val != s,
                    Self::Superset(s) => val.is_superset(s),
                    Self::ProperSuperset(s) => val.is_superset(s) && val != s,
                    Self::Ordered(r) => r.matches(val),
                }
            }
        }
    )+};
}
make_set_restriction!((OrderedSet<String>, String, StrRestrict));
