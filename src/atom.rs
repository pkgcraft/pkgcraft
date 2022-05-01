use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use self::version::ParsedVersion;
pub use self::version::Version;
use crate::eapi::{IntoEapi, EAPI_PKGCRAFT};
use crate::macros::vec_str;
use crate::{Error, Result};
// export parser functionality
pub use parser::parse;

mod parser;
mod version;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum Blocker {
    Strong, // !!cat/pkg
    Weak,   // !cat/pkg
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Blocker::Weak => write!(f, "!"),
            Blocker::Strong => write!(f, "!!"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum Operator {
    Less,           // <cat/pkg-1
    LessOrEqual,    // <=cat/pkg-1
    Equal,          // =cat/pkg-1
    EqualGlob,      // =cat/pkg-1*
    Approximate,    // ~cat/pkg-1
    GreaterOrEqual, // >=cat/pkg-1
    Greater,        // >cat/pkg-1
}

#[derive(Debug)]
pub(crate) struct ParsedAtom<'a> {
    pub(crate) category: &'a str,
    pub(crate) package: &'a str,
    pub(crate) block: Option<Blocker>,
    pub(crate) op: Option<Operator>,
    pub(crate) version: Option<ParsedVersion<'a>>,
    pub(crate) slot: Option<&'a str>,
    pub(crate) subslot: Option<&'a str>,
    pub(crate) slot_op: Option<&'a str>,
    pub(crate) use_deps: Option<Vec<&'a str>>,
    pub(crate) repo: Option<&'a str>,
}

impl ParsedAtom<'_> {
    pub(crate) fn into_owned(self, input: &str) -> Result<Atom> {
        let version = match self.version {
            None => None,
            Some(v) => Some(v.into_owned(input)?),
        };

        Ok(Atom {
            category: self.category.to_string(),
            package: self.package.to_string(),
            block: self.block,
            op: self.op,
            version,
            slot: self.slot.map(|s| s.to_string()),
            subslot: self.subslot.map(|s| s.to_string()),
            slot_op: self.slot_op.map(|s| s.to_string()),
            use_deps: self.use_deps.as_ref().map(|u| vec_str!(u)),
            repo: self.repo.map(|s| s.to_string()),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Atom {
    category: String,
    package: String,
    block: Option<Blocker>,
    op: Option<Operator>,
    version: Option<Version>,
    slot: Option<String>,
    subslot: Option<String>,
    slot_op: Option<String>,
    use_deps: Option<Vec<String>>,
    repo: Option<String>,
}

impl Atom {
    pub fn new<S: AsRef<str>, E: IntoEapi>(s: S, eapi: E) -> Result<Self> {
        parse::dep(s.as_ref(), eapi.into_eapi()?)
    }

    pub fn category(&self) -> &str {
        &self.category
    }

    pub fn package(&self) -> &str {
        &self.package
    }

    pub fn use_deps(&self) -> Option<Vec<&str>> {
        self.use_deps
            .as_ref()
            .map(|u| u.iter().map(|s| s.as_str()).collect())
    }

    pub fn fullver(&self) -> Option<&str> {
        self.version.as_ref().map(|v| v.as_str())
    }

    pub fn key(&self) -> String {
        format!("{}/{}", self.category, self.package)
    }

    pub fn cpv(&self) -> String {
        match &self.version {
            Some(ver) => format!("{}/{}-{ver}", self.category, self.package),
            None => format!("{}/{}", self.category, self.package),
        }
    }

    pub fn slot(&self) -> Option<&str> {
        self.slot.as_deref()
    }

    pub fn subslot(&self) -> Option<&str> {
        self.subslot.as_deref()
    }

    pub fn slot_op(&self) -> Option<&str> {
        self.slot_op.as_deref()
    }

    pub fn repo(&self) -> Option<&str> {
        self.repo.as_deref()
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();

        // append blocker
        if let Some(block) = &self.block {
            s.push_str(&format!("{block}"));
        }

        // append operator and version
        match &self.op {
            Some(Operator::Less) => s.push_str(&format!("<{}", self.cpv())),
            Some(Operator::LessOrEqual) => s.push_str(&format!("<={}", self.cpv())),
            Some(Operator::Equal) => s.push_str(&format!("={}", self.cpv())),
            Some(Operator::EqualGlob) => s.push_str(&format!("={}*", self.cpv())),
            Some(Operator::Approximate) => s.push_str(&format!("~{}", self.cpv())),
            Some(Operator::GreaterOrEqual) => s.push_str(&format!(">={}", self.cpv())),
            Some(Operator::Greater) => s.push_str(&format!(">{}", self.cpv())),
            None => s.push_str(&self.cpv()),
        }

        // append slot data
        match (self.slot(), self.subslot(), self.slot_op()) {
            (Some(slot), Some(subslot), Some(op)) => s.push_str(&format!(":{slot}/{subslot}{op}")),
            (Some(slot), Some(subslot), None) => s.push_str(&format!(":{slot}/{subslot}")),
            (Some(slot), None, Some(op)) => s.push_str(&format!(":{slot}{op}")),
            (Some(x), None, None) | (None, None, Some(x)) => s.push_str(&format!(":{x}")),
            _ => (),
        }

        // append use deps
        if let Some(x) = &self.use_deps {
            s.push_str(&format!("[{}]", &x.join(",")));
        }

        // append repo
        if let Some(repo) = &self.repo {
            s.push_str(&format!("::{repo}"));
        }

        write!(f, "{s}")
    }
}

// Return Ordering if it's not equal.
macro_rules! cmp_not_equal {
    ($x:expr, $y:expr) => {
        let cmp = $x.cmp($y);
        if cmp != Ordering::Equal {
            return cmp;
        }
    };
}
pub(crate) use cmp_not_equal;

impl Ord for Atom {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_not_equal!(&self.category, &other.category);
        cmp_not_equal!(&self.package, &other.package);
        cmp_not_equal!(&self.op, &other.op);
        cmp_not_equal!(&self.version, &other.version);
        cmp_not_equal!(&self.block, &other.block);
        cmp_not_equal!(&self.slot, &other.slot);
        cmp_not_equal!(&self.subslot, &other.subslot);
        cmp_not_equal!(&self.use_deps, &other.use_deps);
        self.repo.cmp(&other.repo)
    }
}

impl PartialOrd for Atom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Atom {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Atom::new(s, &*EAPI_PKGCRAFT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt() {
        let mut atom: Atom;
        for s in [
            "cat/pkg",
            "<cat/pkg-4",
            "<=cat/pkg-4-r1",
            "=cat/pkg-4-r1",
            "=cat/pkg-4*",
            "~cat/pkg-4",
            ">=cat/pkg-r1-2-r3",
            ">cat/pkg-4-r1:0=",
            ">cat/pkg-4-r1:0/2=[use]",
            ">cat/pkg-4-r1:0/2=[use]::repo",
            "!cat/pkg",
            "!!<cat/pkg-4",
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(format!("{atom}"), s);
        }
    }

    #[test]
    fn test_atom_key() {
        let mut atom: Atom;
        for (s, key) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg"),
            ("<=cat/pkg-4-r1", "cat/pkg"),
            ("=cat/pkg-4", "cat/pkg"),
            ("=cat/pkg-4*", "cat/pkg"),
            ("~cat/pkg-4", "cat/pkg"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1"),
            (">cat/pkg-4-r1:0=", "cat/pkg"),
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(atom.key(), key);
        }
    }

    #[test]
    fn test_atom_fullver() {
        let mut atom: Atom;
        for (s, fullver) in [
            ("cat/pkg", None),
            ("<cat/pkg-4", Some("4")),
            ("<=cat/pkg-4-r1", Some("4-r1")),
            ("=cat/pkg-4", Some("4")),
            ("=cat/pkg-4*", Some("4")),
            ("~cat/pkg-4", Some("4")),
            (">=cat/pkg-r1-2-r3", Some("2-r3")),
            (">cat/pkg-4-r1:0=", Some("4-r1")),
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(atom.fullver(), fullver);
        }
    }

    #[test]
    fn test_atom_cpv() {
        let mut atom: Atom;
        for (s, cpv) in [
            ("cat/pkg", "cat/pkg"),
            ("<cat/pkg-4", "cat/pkg-4"),
            ("<=cat/pkg-4-r1", "cat/pkg-4-r1"),
            ("=cat/pkg-4", "cat/pkg-4"),
            ("=cat/pkg-4*", "cat/pkg-4"),
            ("~cat/pkg-4", "cat/pkg-4"),
            (">=cat/pkg-r1-2-r3", "cat/pkg-r1-2-r3"),
            (">cat/pkg-4-r1:0=", "cat/pkg-4-r1"),
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(atom.cpv(), cpv);
        }
    }

    #[test]
    fn test_sorting() {
        for (unsorted, expected) in [
            // categories
            ("c/pkg b/pkg a/pkg", "a/pkg b/pkg c/pkg"),
            // packages
            ("cat/c cat/b cat/a", "cat/a cat/b cat/c"),
            // blocks
            ("!a/b !!a/b a/b", "a/b !!a/b !a/b"),
            // version ops
            (
                ">a/b-1 >=a/b-1 ~a/b-1 =a/b-1* =a/b-1 <=a/b-1 <a/b-1",
                "<a/b-1 <=a/b-1 =a/b-1 =a/b-1* ~a/b-1 >=a/b-1 >a/b-1",
            ),
            // slots
            ("a/b:2 a/b:1 a/b:0", "a/b:0 a/b:1 a/b:2"),
            // subslots
            ("a/b:0/2 a/b:0/1 a/b:0/0", "a/b:0/0 a/b:0/1 a/b:0/2"),
            // use deps
            ("a/b[c] a/b[b] a/b[a]", "a/b[a] a/b[b] a/b[c]"),
            // equal versions shouldn't be sorted
            ("=a/b-0 =a/b-00 =a/b-0-r0", "=a/b-0 =a/b-00 =a/b-0-r0"),
            ("=a/b-1.0.2 =a/b-1.0.2-r0 =a/b-1.000.2", "=a/b-1.0.2 =a/b-1.0.2-r0 =a/b-1.000.2"),
            // simple versions
            ("=a/b-2 =a/b-1 =a/b-0", "=a/b-0 =a/b-1 =a/b-2"),
            ("=a/b-1.100 =a/b-1.10 =a/b-1.1", "=a/b-1.1 =a/b-1.10 =a/b-1.100"),
            // letter suffixes
            ("=a/b-1z =a/b-1y =a/b-1b =a/b-1a", "=a/b-1a =a/b-1b =a/b-1y =a/b-1z"),
            // release suffixes
            (
                "=a/b-1_p =a/b-1_rc =a/b-1_pre =a/b-1_beta =a/b-1_alpha",
                "=a/b-1_alpha =a/b-1_beta =a/b-1_pre =a/b-1_rc =a/b-1_p",
            ),
            ("=a/b-1_p2 =a/b-1_p1 =a/b-1_p0", "=a/b-1_p0 =a/b-1_p1 =a/b-1_p2"),
            // revisions
            ("=a/b-1-r2 =a/b-1-r1 =a/b-1-r0", "=a/b-1-r0 =a/b-1-r1 =a/b-1-r2"),
        ] {
            let mut atoms: Vec<Atom> = unsorted
                .split(' ')
                .map(|s| Atom::from_str(s).unwrap())
                .collect();
            atoms.sort();
            let sorted: Vec<String> = atoms.iter().map(|v| format!("{v}")).collect();
            assert_eq!(sorted.join(" "), expected);
        }
    }
}
