use regex::Regex;

use crate::atom::{Atom, NonOpVersion as no_op, NonRevisionVersion as no_rev, Operator, Version};
use crate::pkg;
use crate::pkg::Package;

#[derive(Debug)]
pub enum AtomAttr {
    Category,
    Package,
    Version,
    Slot,
    SubSlot,
    Repo,
}

impl AtomAttr {
    fn get_value<'a>(&self, atom: &'a Atom) -> &'a str {
        match self {
            AtomAttr::Category => atom.category(),
            AtomAttr::Package => atom.package(),
            AtomAttr::Version => atom.version().map_or_else(|| "", |v| v.as_str()),
            AtomAttr::Slot => atom.slot().unwrap_or_default(),
            AtomAttr::SubSlot => atom.subslot().unwrap_or_default(),
            AtomAttr::Repo => atom.repo().unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub enum Restrict {
    // boolean
    AlwaysTrue,
    AlwaysFalse,

    // atom attributes
    Category(String),
    Package(String),
    Version(Option<Version>),
    Slot(Option<String>),
    SubSlot(Option<String>),
    Use(Vec<String>, Vec<String>),
    Repo(Option<String>),
    PkgAttr(AtomAttr, Box<Restrict>),

    // boolean combinations
    And(Vec<Box<Restrict>>),
    Or(Vec<Box<Restrict>>),

    // string
    StrMatch(String),
    StrPrefix(String),
    StrRegex(Regex),
    StrSuffix(String),
}

pub(crate) trait Restriction<T> {
    fn matches(&self, object: T) -> bool;
}

impl Restriction<&Atom> for Restrict {
    fn matches(&self, atom: &Atom) -> bool {
        match self {
            // boolean
            Restrict::AlwaysTrue => true,

            // atom attributes
            Restrict::Category(s) => s.as_str() == atom.category(),
            Restrict::Package(s) => s.as_str() == atom.package(),
            Restrict::Version(v) => match (v, atom.version()) {
                (Some(v), Some(ver)) => {
                    match v.op() {
                        Some(Operator::Less) => no_op(ver) < no_op(v),
                        Some(Operator::LessOrEqual) => no_op(ver) <= no_op(v),
                        Some(Operator::Equal) | None => no_op(ver) == no_op(v),
                        // TODO: requires string glob restriction support
                        Some(Operator::EqualGlob) => unimplemented!(),
                        Some(Operator::Approximate) => no_rev(ver) == no_rev(v),
                        Some(Operator::GreaterOrEqual) => no_op(ver) >= no_op(v),
                        Some(Operator::Greater) => no_op(ver) > no_op(v),
                    }
                }
                (None, None) => true,
                _ => false,
            },
            Restrict::Slot(s) => s.as_deref() == atom.slot(),
            Restrict::SubSlot(s) => s.as_deref() == atom.subslot(),
            Restrict::Use(_enabled, _disabled) => unimplemented!(),
            Restrict::Repo(s) => s.as_deref() == atom.repo(),

            // package attribute support
            Restrict::PkgAttr(attr, r) => r.matches(attr.get_value(atom)),

            // boolean combinations
            Restrict::And(vals) => vals.iter().all(|r| r.matches(atom)),
            Restrict::Or(vals) => vals.iter().any(|r| r.matches(atom)),

            _ => false,
        }
    }
}

impl Restriction<&str> for Restrict {
    fn matches(&self, val: &str) -> bool {
        match self {
            // boolean
            Restrict::AlwaysTrue => true,

            // string
            Restrict::StrMatch(s) => val == s,
            Restrict::StrPrefix(s) => val.starts_with(s),
            Restrict::StrRegex(re) => re.is_match(val),
            Restrict::StrSuffix(s) => val.ends_with(s),

            _ => false,
        }
    }
}

impl From<&Atom> for Restrict {
    fn from(atom: &Atom) -> Self {
        let mut restricts = vec![
            Box::new(Restrict::Category(atom.category().to_string())),
            Box::new(Restrict::Package(atom.package().to_string())),
        ];

        if let Some(v) = atom.version() {
            let r = match v.op() {
                // equal glob operators are version string prefix checks
                Some(Operator::EqualGlob) => {
                    let r = Box::new(Restrict::StrPrefix(v.as_str().to_string()));
                    Box::new(Restrict::PkgAttr(AtomAttr::Version, r))
                }
                _ => Box::new(Restrict::Version(Some(v.clone()))),
            };
            restricts.push(r);
        }

        if let Some(s) = atom.slot() {
            restricts.push(Box::new(Restrict::Slot(Some(s.to_string()))));
        }

        if let Some(s) = atom.subslot() {
            restricts.push(Box::new(Restrict::SubSlot(Some(s.to_string()))));
        }

        // TODO: add use deps support

        if let Some(s) = atom.repo() {
            restricts.push(Box::new(Restrict::Repo(Some(s.to_string()))));
        }

        Restrict::And(restricts)
    }
}

impl From<&pkg::Pkg<'_>> for Restrict {
    fn from(pkg: &pkg::Pkg) -> Self {
        Restrict::from(pkg.atom())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom::parse;
    use crate::atom::{Atom, Version};

    use super::*;

    #[test]
    fn test_atom_restricts() {
        let unversioned = Atom::from_str("cat/pkg").unwrap();
        let cpv = parse::cpv("cat/pkg-1").unwrap();
        let full = Atom::from_str("=cat/pkg-1:2/3::repo").unwrap();

        // category
        let r = Restrict::Category("cat".to_string());
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // package
        let r = Restrict::Package("pkg".to_string());
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no version
        let r = Restrict::Version(None);
        assert!(r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&full));

        // version
        let r = Restrict::Version(Some(Version::from_str("1").unwrap()));
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no slot
        let r = Restrict::Slot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // slot
        let r = Restrict::Slot(Some("2".to_string()));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no subslot
        let r = Restrict::SubSlot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // subslot
        let r = Restrict::SubSlot(Some("3".to_string()));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no repo
        let r = Restrict::Repo(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // repo
        let r = Restrict::Repo(Some("repo".to_string()));
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

        let r = Restrict::Category("cat".to_string());
        assert_eq!(filter(r, atoms.clone()), atom_strs);

        let r = Restrict::Version(None);
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
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkg".into());
        let r = Restrict::And(vec![Box::new(cat), Box::new(pkg)]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkga".into());
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
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkg".into());
        let r = Restrict::Or(vec![Box::new(cat), Box::new(pkg)]);
        assert!(r.matches(&a));

        // one matched and one unmatched restriction
        let cat = Restrict::Category("cat".into());
        let pkg = Restrict::Package("pkga".into());
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
