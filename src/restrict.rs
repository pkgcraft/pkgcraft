use std::str::FromStr;

use indexmap::IndexSet;
use regex::Regex;

use crate::atom::{NonOpVersion as no_op, NonRevisionVersion as no_rev, Operator as VerOp};
use crate::pkg;
use crate::pkg::Package;
use crate::{atom, Result};

#[derive(Debug)]
pub enum AtomAttr {
    Category(Box<Restrict>),
    Package(Box<Restrict>),
    Version(Option<atom::Version>),
    VersionStr(Box<Restrict>),
    Slot(Box<Restrict>),
    SubSlot(Box<Restrict>),
    StaticUseDep(Box<Restrict>),
    Repo(Box<Restrict>),
}

impl Restriction<&atom::Atom> for AtomAttr {
    fn matches(&self, atom: &atom::Atom) -> bool {
        match self {
            Self::Category(r) => r.matches(atom.category()),
            Self::Package(r) => r.matches(atom.package()),
            Self::Version(v) => match (v, atom.version()) {
                (Some(v), Some(ver)) => {
                    match v.op() {
                        Some(VerOp::Less) => no_op(ver) < no_op(v),
                        Some(VerOp::LessOrEqual) => no_op(ver) <= no_op(v),
                        Some(VerOp::Equal) | None => no_op(ver) == no_op(v),
                        // TODO: requires string glob restriction support
                        Some(VerOp::EqualGlob) => unimplemented!(),
                        Some(VerOp::Approximate) => no_rev(ver) == no_rev(v),
                        Some(VerOp::GreaterOrEqual) => no_op(ver) >= no_op(v),
                        Some(VerOp::Greater) => no_op(ver) > no_op(v),
                    }
                }
                (None, None) => true,
                _ => false,
            },
            Self::VersionStr(r) => r.matches(atom.version().map_or_else(|| "", |v| v.as_str())),
            Self::Slot(r) => r.matches(atom.slot()),
            Self::SubSlot(r) => r.matches(atom.subslot()),
            Self::StaticUseDep(r) => r.matches(&atom.use_deps_set()),
            Self::Repo(r) => r.matches(atom.repo()),
        }
    }
}

#[derive(Debug)]
pub enum PkgAttr {
    Eapi(Box<Restrict>),
}

impl Restriction<&pkg::Pkg<'_>> for PkgAttr {
    fn matches(&self, pkg: &pkg::Pkg) -> bool {
        match self {
            Self::Eapi(r) => r.matches(pkg.eapi().as_str()),
        }
    }
}

#[derive(Debug)]
pub enum Restrict {
    // boolean
    AlwaysTrue,
    AlwaysFalse,

    // object attributes
    Atom(AtomAttr),
    Pkg(PkgAttr),

    // boolean combinations
    And(Vec<Box<Self>>),
    Or(Vec<Box<Self>>),

    // sets
    Contains(IndexSet<String>),

    // strings
    StrMatch(String),
    StrOptional(Option<String>),
    StrPrefix(String),
    StrRegex(Regex),
    StrSuffix(String),
}

impl Restrict {
    pub fn category(s: &str) -> Self {
        let s = s.to_string();
        let r = AtomAttr::Category(Box::new(Self::StrMatch(s)));
        Self::Atom(r)
    }

    pub fn package(s: &str) -> Self {
        let s = s.to_string();
        let r = AtomAttr::Package(Box::new(Self::StrMatch(s)));
        Self::Atom(r)
    }

    pub fn version(o: Option<&str>) -> Result<Self> {
        let o = match o {
            None => None,
            Some(s) => Some(atom::Version::from_str(s)?),
        };
        let r = AtomAttr::Version(o);
        Ok(Self::Atom(r))
    }

    pub fn slot(o: Option<&str>) -> Self {
        let o = o.map(str::to_string);
        let r = AtomAttr::Slot(Box::new(Self::StrOptional(o)));
        Self::Atom(r)
    }

    pub fn subslot(o: Option<&str>) -> Self {
        let o = o.map(str::to_string);
        let r = AtomAttr::SubSlot(Box::new(Self::StrOptional(o)));
        Self::Atom(r)
    }

    pub fn use_deps<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let r = AtomAttr::StaticUseDep(Box::new(Self::Contains(
            iter.into_iter().map(|s| s.into()).collect(),
        )));
        Self::Atom(r)
    }

    pub fn repo(o: Option<&str>) -> Self {
        let o = o.map(str::to_string);
        let r = AtomAttr::Repo(Box::new(Self::StrOptional(o)));
        Self::Atom(r)
    }
}

pub(crate) trait Restriction<T> {
    fn matches(&self, object: T) -> bool;
}

impl Restriction<&atom::Atom> for Restrict {
    fn matches(&self, atom: &atom::Atom) -> bool {
        match self {
            // boolean
            Self::AlwaysTrue => true,

            // object attributes
            Self::Atom(r) => r.matches(atom),

            // boolean combinations
            Self::And(vals) => vals.iter().all(|r| r.matches(atom)),
            Self::Or(vals) => vals.iter().any(|r| r.matches(atom)),

            _ => false,
        }
    }
}

impl Restriction<&str> for Restrict {
    fn matches(&self, val: &str) -> bool {
        match self {
            // boolean
            Self::AlwaysTrue => true,

            // string
            Self::StrMatch(s) => val == s,
            Self::StrPrefix(s) => val.starts_with(s),
            Self::StrRegex(re) => re.is_match(val),
            Self::StrSuffix(s) => val.ends_with(s),

            _ => false,
        }
    }
}

impl Restriction<Option<&str>> for Restrict {
    fn matches(&self, val: Option<&str>) -> bool {
        match self {
            Self::AlwaysTrue => true,
            Self::StrOptional(o) => val == o.as_deref(),
            _ => false,
        }
    }
}

impl Restriction<&IndexSet<&str>> for Restrict {
    fn matches(&self, val: &IndexSet<&str>) -> bool {
        match self {
            Self::AlwaysTrue => true,
            Self::Contains(s) => {
                let set = s.iter().map(|s| s.as_str()).collect::<IndexSet<&str>>();
                set.is_subset(val)
            }
            _ => false,
        }
    }
}

impl From<&atom::Atom> for Restrict {
    fn from(atom: &atom::Atom) -> Self {
        let mut restricts = vec![
            Box::new(Self::category(atom.category())),
            Box::new(Self::package(atom.package())),
        ];

        if let Some(v) = atom.version() {
            let r = match v.op() {
                // equal glob operators are version string prefix checks
                Some(VerOp::EqualGlob) => {
                    AtomAttr::VersionStr(Box::new(Self::StrPrefix(v.as_str().to_string())))
                }
                _ => AtomAttr::Version(Some(v.clone())),
            };
            restricts.push(Box::new(Self::Atom(r)));
        }

        if let Some(s) = atom.slot() {
            restricts.push(Box::new(Self::slot(Some(s))));
        }

        if let Some(s) = atom.subslot() {
            restricts.push(Box::new(Self::subslot(Some(s))));
        }

        // TODO: add use deps support

        if let Some(s) = atom.repo() {
            restricts.push(Box::new(Self::repo(Some(s))));
        }

        Self::And(restricts)
    }
}

impl From<&pkg::Pkg<'_>> for Restrict {
    fn from(pkg: &pkg::Pkg) -> Self {
        Self::from(pkg.atom())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::parse;
    use crate::atom::Atom;

    use super::*;

    #[test]
    fn test_atom_restricts() {
        let unversioned = Atom::from_str("cat/pkg").unwrap();
        let cpv = parse::cpv("cat/pkg-1").unwrap();
        let full = Atom::from_str("=cat/pkg-1:2/3[u1,u2]::repo").unwrap();

        // category
        let r = Restrict::category("cat");
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // package
        let r = Restrict::package("pkg");
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no version
        let r = Restrict::version(None).unwrap();
        assert!(r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&full));

        // version
        let r = Restrict::version(Some("1")).unwrap();
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no slot
        let r = Restrict::slot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // slot
        let r = Restrict::slot(Some("2"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no subslot
        let r = Restrict::subslot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // subslot
        let r = Restrict::subslot(Some("3"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no use deps specified
        let r = Restrict::use_deps([] as [&str; 0]);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // use deps specified
        for u in [vec!["u1"], vec!["u1", "u2"]] {
            let r = Restrict::use_deps(u);
            assert!(!r.matches(&unversioned));
            assert!(!r.matches(&cpv));
            assert!(r.matches(&full));
        }

        // no repo
        let r = Restrict::repo(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // repo
        let r = Restrict::repo(Some("repo"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // unversioned restriction
        let r = Restrict::from(&unversioned);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // cpv restriction
        let r = Restrict::from(&cpv);
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // full atom restriction
        let r = Restrict::from(&full);
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));
    }

    #[test]
    fn test_version_restricts() {
        let lt = Atom::from_str("<cat/pkg-1-r1").unwrap();
        let le = Atom::from_str("<=cat/pkg-1-r1").unwrap();
        let eq = Atom::from_str("=cat/pkg-1-r1").unwrap();
        let eq_glob = Atom::from_str("=cat/pkg-1*").unwrap();
        let approx = Atom::from_str("~cat/pkg-1").unwrap();
        let ge = Atom::from_str(">=cat/pkg-1-r1").unwrap();
        let gt = Atom::from_str(">cat/pkg-1-r1").unwrap();

        let lt_cpv = parse::cpv("cat/pkg-0").unwrap();
        let gt_cpv = parse::cpv("cat/pkg-2").unwrap();

        let r = Restrict::from(&lt);
        assert!(r.matches(&lt_cpv));
        assert!(!r.matches(&lt));
        assert!(!r.matches(&gt_cpv));

        let r = Restrict::from(&le);
        assert!(r.matches(&lt_cpv));
        assert!(r.matches(&le));
        assert!(!r.matches(&gt_cpv));

        let r = Restrict::from(&eq);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq));
        assert!(!r.matches(&gt_cpv));

        let r = Restrict::from(&eq_glob);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq_glob));
        for s in ["cat/pkg-1-r1", "cat/pkg-10", "cat/pkg-1.0.1"] {
            let cpv = parse::cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));
        let r = Restrict::from(&approx);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&approx));
        for s in ["cat/pkg-1-r1", "cat/pkg-1-r999"] {
            let cpv = parse::cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));

        let r = Restrict::from(&ge);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&ge));
        assert!(r.matches(&gt_cpv));

        let r = Restrict::from(&gt);
        assert!(!r.matches(&lt_cpv));
        assert!(!r.matches(&gt));
        assert!(r.matches(&gt_cpv));
    }

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

        let r = Restrict::category("cat");
        assert_eq!(filter(r, atoms.clone()), atom_strs);

        let r = Restrict::version(None).unwrap();
        assert_eq!(filter(r, atoms.clone()), ["cat/pkg"]);

        let cpv = Atom::from_str("=cat/pkg-1").unwrap();
        let r = Restrict::from(&cpv);
        assert_eq!(filter(r, atoms.clone()), [">=cat/pkg-1", "=cat/pkg-1:2/3::repo"]);

        let r = Restrict::AlwaysTrue;
        assert_eq!(filter(r, atoms.clone()), atom_strs);

        let r = Restrict::AlwaysFalse;
        assert!(filter(r, atoms.clone()).is_empty());
    }

    #[test]
    fn test_and_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = Restrict::category("cat");
        let pkg = Restrict::package("pkg");
        let r = Restrict::And(vec![Box::new(cat), Box::new(pkg)]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = Restrict::category("cat");
        let pkg = Restrict::package("pkga");
        let r = Restrict::And(vec![Box::new(cat), Box::new(pkg)]);
        assert!(!r.matches(&a));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = Restrict::And(vec![Box::new(Restrict::from(&a1)), Box::new(Restrict::from(&a2))]);
        assert!(!r.matches(&a1));
        assert!(!r.matches(&a2));
    }

    #[test]
    fn test_or_restrict() {
        let a = Atom::from_str("cat/pkg").unwrap();
        let cat = Restrict::category("cat");
        let pkg = Restrict::package("pkg");
        let r = Restrict::Or(vec![Box::new(cat), Box::new(pkg)]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = Restrict::category("cat");
        let pkg = Restrict::package("pkga");
        let r = Restrict::Or(vec![Box::new(cat), Box::new(pkg)]);
        assert!(r.matches(&a));

        // matching against two atoms
        let a1 = Atom::from_str("cat/pkg1").unwrap();
        let a2 = Atom::from_str("cat/pkg2").unwrap();
        let r = Restrict::Or(vec![Box::new(Restrict::from(&a1)), Box::new(Restrict::from(&a2))]);
        assert!(r.matches(&a1));
        assert!(r.matches(&a2));
    }
}
