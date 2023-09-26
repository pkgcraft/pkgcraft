use crate::dep::{Dep, DepSet, Flatten, Uri};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::Restriction;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict<T> {
    Any(T),
}

// TODO: combine these Restriction implementations using generics
impl Restriction<&DepSet<String, Dep>> for Restrict<DepRestrict> {
    fn matches(&self, val: &DepSet<String, Dep>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DepSet<String, String>> for Restrict<StrRestrict> {
    fn matches(&self, val: &DepSet<String, String>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v)),
        }
    }
}

impl Restriction<&DepSet<String, Uri>> for Restrict<StrRestrict> {
    fn matches(&self, val: &DepSet<String, Uri>) -> bool {
        match self {
            Self::Any(r) => val.into_iter_flatten().any(|v| r.matches(v.as_ref())),
        }
    }
}
