use std::collections::HashSet;
use std::{fmt, ptr};

use regex::Regex;

use crate::{atom, pkg};

// export parser functionality
pub use parser::parse;

mod parser;

#[derive(Debug, Clone)]
pub enum Restrict {
    // boolean
    True,
    False,

    // boolean combinations
    And(Vec<Box<Self>>),
    Or(Vec<Box<Self>>),
    Not(Box<Self>),

    // object attributes
    Atom(atom::Restrict),
    Pkg(pkg::Restrict),

    // sets
    Set(Set),

    // strings
    Str(Str),
}

macro_rules! restrict_match {
   ($r:expr, $obj:expr, $($matcher:pat $(if $pred:expr)* => $result:expr),+) => {
       match $r {
           $($matcher $(if $pred)* => $result,)+

            // boolean
            crate::restrict::Restrict::True => true,
            crate::restrict::Restrict::False => false,

            // boolean combinations
            crate::restrict::Restrict::And(vals) => vals.iter().all(|r| r.matches($obj)),
            crate::restrict::Restrict::Or(vals) => vals.iter().any(|r| r.matches($obj)),
            crate::restrict::Restrict::Not(r) => !r.matches($obj),

            _ => {
                tracing::warn!("invalid restriction {:?} for matching {:?}", $r, $obj);
                false
            }
       }
   }
}
pub(crate) use restrict_match;

impl Restrict {
    pub fn and<I, T>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Restrict>,
    {
        Self::And(iter.into_iter().map(|x| Box::new(x.into())).collect())
    }

    pub fn or<I, T>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Restrict>,
    {
        Self::Or(iter.into_iter().map(|x| Box::new(x.into())).collect())
    }

    pub fn not<T>(obj: T) -> Self
    where
        T: Into<Restrict>,
    {
        Self::Not(Box::new(obj.into()))
    }
}

pub(crate) trait Restriction<T> {
    fn matches(&self, object: T) -> bool;
}

impl Restriction<&str> for Restrict {
    fn matches(&self, s: &str) -> bool {
        restrict_match! {
            self, s,
            Self::Str(r) => r.matches(s)
        }
    }
}

#[derive(Clone)]
pub enum Str {
    Custom(fn(&str) -> bool),
    Matches(String),
    Prefix(String),
    Regex(Regex),
    Substr(String),
    Suffix(String),
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(func) => write!(f, "Custom(func: {:?})", ptr::addr_of!(func)),
            Self::Matches(s) => write!(f, "Matches({s:?})"),
            Self::Prefix(s) => write!(f, "Prefix({s:?})"),
            Self::Regex(re) => write!(f, "Regex({re:?})"),
            Self::Substr(s) => write!(f, "Substr({s:?})"),
            Self::Suffix(s) => write!(f, "Suffix({s:?})"),
        }
    }
}

impl Str {
    pub fn custom(f: fn(&str) -> bool) -> Self {
        Self::Custom(f)
    }

    pub fn matches<S: Into<String>>(s: S) -> Self {
        Self::Matches(s.into())
    }

    pub fn prefix<S: Into<String>>(s: S) -> Self {
        Self::Prefix(s.into())
    }

    pub fn regex(re: Regex) -> Self {
        Self::Regex(re)
    }

    pub fn substr<S: Into<String>>(s: S) -> Self {
        Self::Substr(s.into())
    }

    pub fn suffix<S: Into<String>>(s: S) -> Self {
        Self::Suffix(s.into())
    }
}

impl Restriction<&str> for Str {
    fn matches(&self, val: &str) -> bool {
        match self {
            Self::Custom(func) => func(val),
            Self::Matches(s) => val == s,
            Self::Prefix(s) => val.starts_with(s),
            Self::Regex(re) => re.is_match(val),
            Self::Substr(s) => val.contains(s),
            Self::Suffix(s) => val.ends_with(s),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Set {
    Empty,
    StrSubset(HashSet<String>),
}

impl Restriction<&HashSet<String>> for Set {
    fn matches(&self, val: &HashSet<String>) -> bool {
        match self {
            Self::Empty => val.is_empty(),
            Self::StrSubset(s) => s.is_subset(val),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::Atom;

    use super::*;

    #[test]
    fn test_filtering() {
        let atom_strs = vec!["cat/pkg", ">=cat/pkg-1", "=cat/pkg-1:2/3::repo"];
        let atoms: Vec<Atom> = atom_strs
            .iter()
            .map(|s| Atom::from_str(s).unwrap())
            .collect();

        let filter = |r: Restrict, atoms: Vec<Atom>| -> Vec<String> {
            atoms
                .into_iter()
                .filter(|a| r.matches(a))
                .map(|a| a.to_string())
                .collect()
        };

        let r = Restrict::Atom(atom::Restrict::category("cat"));
        assert_eq!(filter(r, atoms.clone()), atom_strs);

        let r = Restrict::Atom(atom::Restrict::version(None).unwrap());
        assert_eq!(filter(r, atoms.clone()), ["cat/pkg"]);

        let cpv = Atom::from_str("=cat/pkg-1").unwrap();
        let r = Restrict::from(&cpv);
        assert_eq!(filter(r, atoms.clone()), [">=cat/pkg-1", "=cat/pkg-1:2/3::repo"]);

        let r = Restrict::True;
        assert_eq!(filter(r, atoms.clone()), atom_strs);

        let r = Restrict::False;
        assert!(filter(r, atoms.clone()).is_empty());
    }

    #[test]
    fn test_and_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkg");
        let r = Restrict::and([cat, pkg]);
        assert!(r.matches(&a));
        let not_r = Restrict::not(r);
        assert!(!not_r.matches(&a));

        // one matched and one unmatched restriction
        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkga");
        let r = Restrict::and([cat, pkg]);
        assert!(!r.matches(&a));
        let not_r = Restrict::not(r);
        assert!(not_r.matches(&a));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = Restrict::and([&a1, &a2]);
        assert!(!r.matches(&a1));
        assert!(!r.matches(&a2));
        let not_r = Restrict::not(r);
        assert!(not_r.matches(&a1));
        assert!(not_r.matches(&a2));
    }

    #[test]
    fn test_or_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkg");
        let r = Restrict::or([cat, pkg]);
        assert!(r.matches(&a));
        let not_r = Restrict::not(r);
        assert!(!not_r.matches(&a));

        // one matched and one unmatched restriction
        let cat = atom::Restrict::category("cat");
        let pkg = atom::Restrict::package("pkga");
        let r = Restrict::or([cat, pkg]);
        assert!(r.matches(&a));
        let not_r = Restrict::not(r);
        assert!(!not_r.matches(&a));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = Restrict::or([&a1, &a2]);
        assert!(r.matches(&a1));
        assert!(r.matches(&a2));
        let not_r = Restrict::not(r);
        assert!(!not_r.matches(&a1));
        assert!(!not_r.matches(&a2));
    }

    #[test]
    fn test_not_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat_r = atom::Restrict::category("cat1");

        // restrict matches
        let r = Restrict::not(cat_r.clone());
        assert!(r.matches(&a));

        // inverse doesn't match
        let r = Restrict::not(r);
        assert!(!r.matches(&a));
    }

    #[test]
    fn test_str_restrict() {
        // custom
        let f = |s: &str| -> bool { s == "a" };
        let r = Str::custom(f);
        assert!(r.matches("a"));
        assert!(!r.matches("b"));

        // matches
        let r = Str::matches("a");
        assert!(r.matches("a"));
        assert!(!r.matches("b"));

        // prefix
        let r = Str::prefix("ab");
        assert!(r.matches("ab"));
        assert!(r.matches("abc"));
        assert!(!r.matches("a"));
        assert!(!r.matches("cab"));

        // regex
        let re = Regex::new("^(a|b)$").unwrap();
        let r = Str::regex(re);
        assert!(r.matches("a"));
        assert!(r.matches("b"));
        assert!(!r.matches("ab"));

        // substr
        let r = Str::substr("ab");
        assert!(r.matches("ab"));
        assert!(r.matches("cab"));
        assert!(r.matches("cabo"));
        assert!(!r.matches("acb"));

        // suffix
        let r = Str::suffix("ab");
        assert!(r.matches("ab"));
        assert!(r.matches("cab"));
        assert!(!r.matches("a"));
        assert!(!r.matches("abc"));
    }
}
