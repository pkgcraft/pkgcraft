use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

pub use self::version::Version;
use crate::eapi;
// export parser functionality
pub use parser::parse;

mod parser;
mod version;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub enum Operator {
    Less,           // <cat/pkg-1
    LessOrEqual,    // <=cat/pkg-1
    Equal,          // =cat/pkg-1
    EqualGlob,      // =cat/pkg-1*
    Approximate,    // ~cat/pkg-1
    GreaterOrEqual, // >=cat/pkg-1
    Greater,        // >cat/pkg-1
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Atom {
    pub category: String,
    pub package: String,
    pub block: Option<Blocker>,
    pub op: Option<Operator>,
    pub version: Option<Version>,
    pub slot: Option<String>,
    pub subslot: Option<String>,
    pub slot_op: Option<String>,
    pub use_deps: Option<Vec<String>>,
    pub repo: Option<String>,
}

impl Atom {
    pub fn fullver(&self) -> Option<String> {
        self.version.as_ref().map(|ver| format!("{}", ver))
    }

    pub fn key(&self) -> String {
        format!("{}/{}", self.category, self.package)
    }

    pub fn cpv(&self) -> String {
        match &self.version {
            Some(ver) => format!("{}/{}-{}", self.category, self.package, ver),
            None => format!("{}/{}", self.category, self.package),
        }
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();

        // append blocker
        if let Some(block) = &self.block {
            s.push_str(&format!("{}", block));
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
        match (&self.slot, &self.subslot, &self.slot_op) {
            (Some(slot), Some(subslot), Some(op)) => {
                s.push_str(&format!(":{}/{}{}", slot, subslot, op))
            }
            (Some(slot), Some(subslot), None) => s.push_str(&format!(":{}/{}", slot, subslot)),
            (Some(slot), None, Some(op)) => s.push_str(&format!(":{}{}", slot, op)),
            (Some(x), None, None) | (None, None, Some(x)) => s.push_str(&format!(":{}", x)),
            _ => (),
        }

        // append use deps
        if let Some(x) = &self.use_deps {
            s.push_str(&format!("[{}]", &x.join(",")));
        }

        // append repo
        if let Some(repo) = &self.repo {
            s.push_str(&format!("::{}", repo));
        }

        write!(f, "{}", s)
    }
}

impl Ord for Atom {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut cmp: Ordering;

        cmp = self.category.cmp(&other.category);
        if cmp != Ordering::Equal {
            return cmp;
        }

        cmp = self.package.cmp(&other.package);
        if cmp != Ordering::Equal {
            return cmp;
        }

        cmp = self.op.cmp(&other.op);
        if cmp != Ordering::Equal {
            return cmp;
        }

        cmp = self.version.cmp(&other.version);
        if cmp != Ordering::Equal {
            return cmp;
        }

        cmp = self.block.cmp(&other.block);
        if cmp != Ordering::Equal {
            return cmp;
        }

        cmp = self.slot.cmp(&other.slot);
        if cmp != Ordering::Equal {
            return cmp;
        }

        cmp = self.subslot.cmp(&other.subslot);
        if cmp != Ordering::Equal {
            return cmp;
        }

        cmp = self.use_deps.cmp(&other.use_deps);
        if cmp != Ordering::Equal {
            return cmp;
        }

        self.repo.cmp(&other.repo)
    }
}

impl PartialOrd for Atom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Atom {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse::dep(s, eapi::EAPI_LATEST)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::opt_str;

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
            "!cat/pkg",
            "!!<cat/pkg-4",
        ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(format!("{}", atom), s);
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
            ("<cat/pkg-4", opt_str!("4")),
            ("<=cat/pkg-4-r1", opt_str!("4-r1")),
            ("=cat/pkg-4", opt_str!("4")),
            ("=cat/pkg-4*", opt_str!("4")),
            ("~cat/pkg-4", opt_str!("4")),
            (">=cat/pkg-r1-2-r3", opt_str!("2-r3")),
            (">cat/pkg-4-r1:0=", opt_str!("4-r1")),
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
            ("!a/b !!a/b", "!!a/b !a/b"),
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
            (
                "=a/b-1.0.2 =a/b-1.0.2-r0 =a/b-1.000.2",
                "=a/b-1.0.2 =a/b-1.0.2-r0 =a/b-1.000.2",
            ),
            // simple versions
            ("=a/b-2 =a/b-1 =a/b-0", "=a/b-0 =a/b-1 =a/b-2"),
            (
                "=a/b-1.100 =a/b-1.10 =a/b-1.1",
                "=a/b-1.1 =a/b-1.10 =a/b-1.100",
            ),
            // letter suffixes
            (
                "=a/b-1z =a/b-1y =a/b-1b =a/b-1a",
                "=a/b-1a =a/b-1b =a/b-1y =a/b-1z",
            ),
            // release suffixes
            (
                "=a/b-1_p =a/b-1_rc =a/b-1_pre =a/b-1_beta =a/b-1_alpha",
                "=a/b-1_alpha =a/b-1_beta =a/b-1_pre =a/b-1_rc =a/b-1_p",
            ),
            (
                "=a/b-1_p2 =a/b-1_p1 =a/b-1_p0",
                "=a/b-1_p0 =a/b-1_p1 =a/b-1_p2",
            ),
            // revisions
            (
                "=a/b-1-r2 =a/b-1-r1 =a/b-1-r0",
                "=a/b-1-r0 =a/b-1-r1 =a/b-1-r2",
            ),
        ] {
            let mut atoms: Vec<Atom> = unsorted
                .split(' ')
                .map(|s| Atom::from_str(s).unwrap())
                .collect();
            atoms.sort();
            let sorted: Vec<String> = atoms.iter().map(|v| format!("{}", v)).collect();
            assert_eq!(sorted.join(" "), expected);
        }
    }
}
