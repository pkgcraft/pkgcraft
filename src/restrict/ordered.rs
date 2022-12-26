use std::cmp::Ordering;

use crate::set::OrderedSet;

use super::str::Restrict as StrRestrict;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict<R> {
    Any(R),
    All(R),
    First(R),
    Last(R),
    Count(Vec<Ordering>, usize),
}

macro_rules! make_ordered_restrictions {
    ($(($x:ty, $r:ty)),+) => {$(
        impl crate::restrict::Restriction<$x> for crate::restrict::ordered::Restrict<$r> {
            fn matches(&self, val: $x) -> bool {
                match self {
                    Self::Any(r) => val.iter().any(|v| r.matches(v)),
                    Self::All(r) => val.iter().all(|v| r.matches(v)),
                    Self::First(r) => val.first().map(|v| r.matches(v)).unwrap_or_default(),
                    Self::Last(r) => val.last().map(|v| r.matches(v)).unwrap_or_default(),
                    Self::Count(ordering, size) => ordering.contains(&val.len().cmp(size)),
                }
            }
        }
    )+};
}
pub(crate) use make_ordered_restrictions;
make_ordered_restrictions!((&[String], StrRestrict), (&OrderedSet<String>, StrRestrict));
