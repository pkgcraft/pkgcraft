use std::str::FromStr;

use crate::restrict::{self, *};

use super::{Atom, Blocker, Version};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict {
    Category(Str),
    Package(Str),
    Blocker(Option<Blocker>),
    Version(Option<Version>),
    VersionStr(Str),
    Slot(Option<Str>),
    Subslot(Option<Str>),
    UseDeps(Option<OrderedSetRestrict<String, Str>>),
    Repo(Option<Str>),
}

impl Restrict {
    pub fn category(s: &str) -> Self {
        Self::Category(Str::equal(s))
    }

    pub fn package(s: &str) -> Self {
        Self::Package(Str::equal(s))
    }

    pub fn version(s: &str) -> crate::Result<Self> {
        let v = Version::from_str(s)?;
        Ok(Self::Version(Some(v)))
    }

    pub fn slot(o: Option<&str>) -> Self {
        Self::Slot(o.map(Str::equal))
    }

    pub fn subslot(o: Option<&str>) -> Self {
        Self::Subslot(o.map(Str::equal))
    }

    pub fn use_deps<I, S>(iter: Option<I>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        match iter {
            None => Self::UseDeps(None),
            Some(i) => {
                let r = OrderedSetRestrict::Superset(i.into_iter().map(|s| s.into()).collect());
                Self::UseDeps(Some(r))
            }
        }
    }

    pub fn repo(o: Option<&str>) -> Self {
        Self::Repo(o.map(Str::equal))
    }
}

impl Restriction<&Atom> for Restrict {
    fn matches(&self, atom: &Atom) -> bool {
        use self::Restrict::*;
        match self {
            Category(r) => r.matches(atom.category()),
            Package(r) => r.matches(atom.package()),
            Blocker(b) => match (b, atom.blocker()) {
                (Some(b), Some(blocker)) => *b == blocker,
                (None, None) => true,
                _ => false,
            },
            Version(v) => match (v, atom.version()) {
                (Some(v), Some(ver)) => v.op_cmp(ver),
                (None, None) => true,
                _ => false,
            },
            VersionStr(r) => r.matches(atom.version().map_or_else(|| "", |v| v.as_str())),
            Slot(r) => match (r, atom.slot()) {
                (Some(r), Some(slot)) => r.matches(slot),
                (None, None) => true,
                _ => false,
            },
            Subslot(r) => match (r, atom.subslot()) {
                (Some(r), Some(subslot)) => r.matches(subslot),
                (None, None) => true,
                _ => false,
            },
            UseDeps(r) => match (r, atom.use_deps()) {
                (Some(r), Some(vals)) => r.matches(vals),
                (None, None) => true,
                _ => false,
            },
            Repo(r) => match (r, atom.repo()) {
                (Some(r), Some(repo)) => r.matches(repo),
                (None, None) => true,
                _ => false,
            },
        }
    }
}

impl From<Restrict> for restrict::Restrict {
    fn from(r: Restrict) -> Self {
        Self::Atom(r)
    }
}

impl Restriction<&Atom> for restrict::Restrict {
    fn matches(&self, atom: &Atom) -> bool {
        crate::restrict::restrict_match! {self, atom,
            Self::Atom(r) => r.matches(atom),
        }
    }
}

impl From<&Atom> for restrict::Restrict {
    fn from(atom: &Atom) -> Self {
        let mut restricts = vec![
            Restrict::category(atom.category()),
            Restrict::package(atom.package()),
            Restrict::Blocker(atom.blocker()),
        ];

        if let Some(v) = atom.version() {
            restricts.push(Restrict::Version(Some(v.clone())));
        }

        if let Some(s) = atom.slot() {
            restricts.push(Restrict::slot(Some(s)));
        }

        if let Some(s) = atom.subslot() {
            restricts.push(Restrict::subslot(Some(s)));
        }

        if let Some(u) = atom.use_deps() {
            restricts.push(Restrict::use_deps(Some(u)));
        }

        if let Some(s) = atom.repo() {
            restricts.push(Restrict::repo(Some(s)));
        }

        restrict::Restrict::and(restricts)
    }
}

#[cfg(test)]
mod tests {
    use crate::atom::cpv;

    use super::*;

    #[test]
    fn test_restrict_methods() {
        let unversioned = Atom::from_str("cat/pkg").unwrap();
        let blocker = Atom::from_str("!cat/pkg").unwrap();
        let cpv = cpv("cat/pkg-1").unwrap();
        let full = Atom::from_str("=cat/pkg-1:2/3[u1,u2]::repo").unwrap();

        // category
        let r = Restrict::category("cat");
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // package
        let r = Restrict::package("pkg");
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // blocker
        let r = Restrict::Blocker(None);
        assert!(r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));
        let r = Restrict::Blocker(Some(Blocker::Weak));
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&full));

        // no version
        let r = Restrict::Version(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&full));

        // version
        let r = Restrict::version("1").unwrap();
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // no slot
        let r = Restrict::slot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // slot
        let r = Restrict::slot(Some("2"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no subslot
        let r = Restrict::subslot(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // subslot
        let r = Restrict::subslot(Some("3"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));

        // no use deps specified
        let r = Restrict::use_deps(None::<&[String]>);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // use deps specified
        for u in [vec!["u1"], vec!["u1", "u2"]] {
            let r = Restrict::use_deps(Some(u));
            assert!(!r.matches(&unversioned));
            assert!(!r.matches(&blocker));
            assert!(!r.matches(&cpv));
            assert!(r.matches(&full));
        }

        // no repo
        let r = Restrict::repo(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // repo
        let r = Restrict::repo(Some("repo"));
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));
    }

    #[test]
    fn test_restrict_conversion() {
        let unversioned = Atom::from_str("cat/pkg").unwrap();
        let cpv = cpv("cat/pkg-1").unwrap();
        let full = Atom::from_str("=cat/pkg-1:2/3[u1,u2]::repo").unwrap();

        // unversioned restriction
        let r = restrict::Restrict::from(&unversioned);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // cpv restriction
        let r = restrict::Restrict::from(&cpv);
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // full atom restriction
        let r = restrict::Restrict::from(&full);
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));
    }

    #[test]
    fn test_restrict_versions() {
        let lt = Atom::from_str("<cat/pkg-1-r1").unwrap();
        let le = Atom::from_str("<=cat/pkg-1-r1").unwrap();
        let eq = Atom::from_str("=cat/pkg-1-r1").unwrap();
        let eq_glob = Atom::from_str("=cat/pkg-1*").unwrap();
        let approx = Atom::from_str("~cat/pkg-1").unwrap();
        let ge = Atom::from_str(">=cat/pkg-1-r1").unwrap();
        let gt = Atom::from_str(">cat/pkg-1-r1").unwrap();

        let lt_cpv = cpv("cat/pkg-0").unwrap();
        let gt_cpv = cpv("cat/pkg-2").unwrap();

        let r = restrict::Restrict::from(&lt);
        assert!(r.matches(&lt_cpv));
        assert!(!r.matches(&lt));
        assert!(!r.matches(&gt_cpv));

        let r = restrict::Restrict::from(&le);
        assert!(r.matches(&lt_cpv));
        assert!(r.matches(&le));
        assert!(!r.matches(&gt_cpv));

        let r = restrict::Restrict::from(&eq);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq));
        assert!(!r.matches(&gt_cpv));

        let r = restrict::Restrict::from(&eq_glob);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq_glob));
        for s in ["cat/pkg-1-r1", "cat/pkg-10", "cat/pkg-1.0.1"] {
            let cpv = cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));
        let r = restrict::Restrict::from(&approx);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&approx));
        for s in ["cat/pkg-1-r1", "cat/pkg-1-r999"] {
            let cpv = cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));

        let r = restrict::Restrict::from(&ge);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&ge));
        assert!(r.matches(&gt_cpv));

        let r = restrict::Restrict::from(&gt);
        assert!(!r.matches(&lt_cpv));
        assert!(!r.matches(&gt));
        assert!(r.matches(&gt_cpv));
    }
}
