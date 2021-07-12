use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use self::version::Version;
use crate::eapi;

mod parser;
mod version;

pub type ParseError = ::peg::error::ParseError<::peg::str::LineCol>;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Blocker {
    Strong, // !!cat/pkg
    Weak, // !cat/pkg
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Blocker::Weak => write!(f, "!"),
            Blocker::Strong => write!(f, "!!"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Operator {
    LT, // <
    LE, // <=
    EQ, // =
    IR, // ~
    GE, // >=
    GT, // >
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Operator::LT => write!(f, "<"),
            Operator::LE => write!(f, "<="),
            Operator::EQ => write!(f, "="),
            Operator::IR => write!(f, "~"),
            Operator::GE => write!(f, ">="),
            Operator::GT => write!(f, ">"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
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
}

impl Atom {
    pub fn fullver(&self) -> Option<String> {
        match &self.version {
            Some(ver) => Some(format!("{}", ver)),
            None => None,
        }
    }

    pub fn key(&self) -> String {
        format!("{}/{}", self.category, self.package)
    }

    pub fn cpv(&self) -> String {
        let mut s = format!("{}/{}", self.category, self.package);
        if let Some(ver) = &self.version {
            s.push_str(&format!("-{}", ver));
        }
        s
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::new();

        // append blocker
        if let Some(block) = &self.block {
            s.push_str(&format!("{}", block));
        }

        // append operator
        if let Some(op) = &self.op {
            s.push_str(&format!("{}", op));
        }

        s.push_str(&self.cpv());

        // append slot data
        if let Some(slot) = &self.slot {
            s.push_str(&format!(":{}", slot));
            if let Some(subslot) = &self.subslot {
                s.push_str(&format!("/{}", subslot));
            }
            if let Some(slot_op) = &self.slot_op {
                s.push_str(slot_op);
            }
        } else if let Some(slot_op) = &self.slot_op {
            s.push_str(&format!(":{}", slot_op));
        }

        // append use deps
        if let Some(x) = &self.use_deps {
            s.push_str(&format!("[{}]", &x.join(",")));
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

        self.use_deps.cmp(&other.use_deps)
    }
}

impl PartialOrd for Atom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

pub fn parse(s: &str, eapi: &'static eapi::Eapi) -> Result<Atom, ParseError> {
    parser::pkg::atom(s, &eapi)
}

impl FromStr for Atom {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parser::pkg::atom(s, eapi::EAPI_LATEST)
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
                "~cat/pkg-4",
                ">=cat/pkg-r1-2-r3",
                ">cat/pkg-4-r1:0=",
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
                (">cat/pkg-4", "cat/pkg-4"),
                (">cat/pkg-4-r1", "cat/pkg-4-r1"),
                (">cat/pkg-r1-2-r3", "cat/pkg-r1-2-r3"),
                (">cat/pkg-4-r1:0=", "cat/pkg-4-r1"),
                ] {
            atom = Atom::from_str(&s).unwrap();
            assert_eq!(atom.cpv(), cpv);
        }
    }
}
