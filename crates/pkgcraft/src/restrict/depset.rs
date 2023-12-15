use crate::dep::{Dep, DependencySet, Flatten, Uri};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::Restriction;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict<T> {
    Any(T),
}

// TODO: combine these Restriction implementations using generics
impl Restriction<&DependencySet<String, Dep<String>>> for Restrict<DepRestrict> {
    fn matches(&self, val: &DependencySet<String, Dep<String>>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DependencySet<String, String>> for Restrict<StrRestrict> {
    fn matches(&self, val: &DependencySet<String, String>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DependencySet<String, Uri>> for Restrict<StrRestrict> {
    fn matches(&self, val: &DependencySet<String, Uri>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v.as_ref())),
        }
    }
}
