use std::fmt;

use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

use crate::pkg::Restrict as PkgRestrict;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::types::Deque;

pub(crate) mod boolean;
pub mod dep;
pub mod depset;
pub mod ordered;
pub mod parse;
pub mod set;
pub mod str;

boolean::restrict_with_boolean! {Restrict,
    // constants
    True,
    False,

    // object attributes
    Dep(DepRestrict),
    Pkg(PkgRestrict),

    // strings
    Str(str::Restrict),
}

impl Default for Restrict {
    fn default() -> Self {
        Self::False
    }
}

impl From<&Restrict> for Restrict {
    fn from(r: &Restrict) -> Self {
        r.clone()
    }
}

pub trait TryIntoRestrict<C> {
    fn try_into_restrict(self, context: &C) -> crate::Result<Restrict>;
}

macro_rules! make_try_into_restrict {
    ($($x:ty),+) => {$(
        impl<C> TryIntoRestrict<C> for $x {
            fn try_into_restrict(self, _context: &C) -> crate::Result<Restrict> {
                Ok(self.into())
            }
        }
    )+};
}

make_try_into_restrict!(&crate::dep::Dep, Restrict, &Restrict);

impl<C> TryIntoRestrict<C> for &str {
    fn try_into_restrict(self, _context: &C) -> crate::Result<Restrict> {
        parse::dep(self)
    }
}

impl Restrict {
    /// Flatten a restriction, returning an iterator of its component restrictions.
    pub fn iter_flatten(&self) -> IterFlatten {
        IterFlatten([self].into_iter().collect())
    }
}

#[derive(Debug)]
pub struct IterFlatten<'a>(Deque<&'a Restrict>);

impl<'a> Iterator for IterFlatten<'a> {
    type Item = &'a Restrict;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(restrict) = self.0.pop_front() {
            match restrict {
                Restrict::And(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                Restrict::Or(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                Restrict::Xor(vals) => self.0.extend_left(vals.iter().map(AsRef::as_ref)),
                _ => return Some(restrict),
            }
        }
        None
    }
}

macro_rules! restrict_match {
   ($r:expr, $obj:expr, $($matcher:pat $(if $pred:expr)* => $result:expr,)+) => {
       match $r {
           $($matcher $(if $pred)* => $result,)+

            // boolean
            Self::True => true,
            Self::False => false,

            // boolean combinations
            Self::And(vals) => vals.iter().all(|r| r.matches($obj)),
            Self::Or(vals) => vals.iter().any(|r| r.matches($obj)),
            Self::Xor(vals) => {
                let mut curr: Option<bool>;
                let mut prev: Option<bool> = None;
                for r in vals {
                    curr = Some(r.matches($obj));
                    if prev.is_some() && curr != prev {
                        return true;
                    }
                    prev = curr
                }
                false
            },
            Self::Not(r) => !r.matches($obj),

            _ => {
                tracing::warn!("invalid restriction {:?} for matching {:?}", $r, $obj);
                false
            }
       }
   }
}
pub(crate) use restrict_match;

impl Restrict {
    boolean::restrict_impl_boolean! {Self}
}

boolean::restrict_ops_boolean!(Restrict);

pub trait Restriction<T>: fmt::Debug {
    fn matches(&self, object: T) -> bool;
}

impl Restriction<&String> for Restrict {
    fn matches(&self, s: &String) -> bool {
        restrict_match! {self, s,
            Self::Dep(r) => r.matches(s.as_str()),
            Self::Str(r) => r.matches(s.as_str()),
        }
    }
}

impl Restriction<&str> for Restrict {
    fn matches(&self, s: &str) -> bool {
        restrict_match! {self, s,
            Self::Dep(r) => r.matches(s),
            Self::Str(r) => r.matches(s),
        }
    }
}

/// Defines the scope for restriction matches.
#[derive(
    AsRefStr,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
)]
#[strum(serialize_all = "kebab-case")]
pub enum Scope {
    /// Single package version target.
    Version,
    /// Single unversioned package target.
    Package,
    /// Multiple unversioned package or category targets.
    Category,
    /// Full repo target.
    Repo,
}

impl From<&Restrict> for Scope {
    fn from(value: &Restrict) -> Self {
        use dep::Restrict::{Category, Package, Version};
        use str::Restrict::Equal;

        let restrict_scope = |restrict: &Restrict| match restrict {
            Restrict::Dep(Version(Some(_))) => Scope::Version,
            Restrict::Dep(Package(Equal(_))) => Scope::Package,
            Restrict::Dep(Package(_)) => Scope::Category,
            Restrict::Dep(Category(_)) => Scope::Category,
            _ => Scope::Repo,
        };

        match value {
            Restrict::And(_) => value
                .iter_flatten()
                .map(restrict_scope)
                .min()
                .unwrap_or(Scope::Repo),
            Restrict::Or(_) => value
                .iter_flatten()
                .map(restrict_scope)
                .max()
                .unwrap_or(Scope::Repo),
            _ => restrict_scope(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dep::Dep;

    use super::*;

    #[test]
    fn filtering() {
        let dep_strs = vec!["cat/pkg", ">=cat/pkg-1", "=cat/pkg-1:2/3::repo"];
        let deps: Vec<_> = dep_strs.iter().map(|s| s.parse().unwrap()).collect();

        let filter = |r: Restrict, deps: Vec<Dep>| -> Vec<String> {
            deps.into_iter()
                .filter(|a| r.matches(a))
                .map(|a| a.to_string())
                .collect()
        };

        let r = Restrict::Dep(dep::Restrict::category("cat"));
        assert_eq!(filter(r, deps.clone()), dep_strs);

        let r = Restrict::Dep(dep::Restrict::Version(None));
        assert_eq!(filter(r, deps.clone()), ["cat/pkg"]);

        let dep = Dep::try_new("=cat/pkg-1").unwrap();
        let r = Restrict::from(&dep);
        assert_eq!(filter(r, deps.clone()), [">=cat/pkg-1", "=cat/pkg-1:2/3::repo"]);

        let r = Restrict::True;
        assert_eq!(filter(r, deps.clone()), dep_strs);

        let r = Restrict::False;
        assert!(filter(r, deps).is_empty());
    }
}
