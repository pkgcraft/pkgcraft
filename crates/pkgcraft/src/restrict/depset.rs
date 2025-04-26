use crate::dep::{Dep, DependencySet, Flatten, Recursive, Uri};
use crate::restrict::Restriction;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict<T> {
    Any(T),
    Contains(StrRestrict),
}

impl Restriction<&DependencySet<Dep>> for Restrict<DepRestrict> {
    fn matches(&self, val: &DependencySet<Dep>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v)),
            Self::Contains(r) => val.into_iter_recursive().any(|v| r.matches(&v.to_string())),
        }
    }
}

impl Restriction<DependencySet<&Dep>> for Restrict<DepRestrict> {
    fn matches(&self, val: DependencySet<&Dep>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v)),
            Self::Contains(r) => val.into_iter_recursive().any(|v| r.matches(&v.to_string())),
        }
    }
}

impl Restriction<&DependencySet<String>> for Restrict<StrRestrict> {
    fn matches(&self, val: &DependencySet<String>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v)),
            Self::Contains(r) => val.into_iter_recursive().any(|v| r.matches(&v.to_string())),
        }
    }
}

impl Restriction<&DependencySet<Uri>> for Restrict<StrRestrict> {
    fn matches(&self, val: &DependencySet<Uri>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v.as_ref())),
            Self::Contains(r) => val.into_iter_recursive().any(|v| r.matches(&v.to_string())),
        }
    }
}
