use std::fmt;

use crate::pkg::Restrict as PkgRestrict;
use crate::restrict::dep::Restrict as DepRestrict;

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

impl From<&Restrict> for Restrict {
    fn from(r: &Restrict) -> Self {
        r.clone()
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
                for r in vals.iter() {
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

impl Restriction<&str> for Restrict {
    fn matches(&self, s: &str) -> bool {
        restrict_match! {self, s,
            Self::Str(r) => r.matches(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::dep::Dep;

    use super::*;

    #[test]
    fn test_filtering() {
        let dep_strs = vec!["cat/pkg", ">=cat/pkg-1", "=cat/pkg-1:2/3::repo"];
        let deps: Vec<Dep> = dep_strs.iter().map(|s| Dep::from_str(s).unwrap()).collect();

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

        let cpv = Dep::from_str("=cat/pkg-1").unwrap();
        let r = Restrict::from(&cpv);
        assert_eq!(filter(r, deps.clone()), [">=cat/pkg-1", "=cat/pkg-1:2/3::repo"]);

        let r = Restrict::True;
        assert_eq!(filter(r, deps.clone()), dep_strs);

        let r = Restrict::False;
        assert!(filter(r, deps).is_empty());
    }
}
