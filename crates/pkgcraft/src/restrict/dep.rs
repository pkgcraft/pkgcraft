use std::str::FromStr;

use crate::dep::{Blocker, Dep, Intersects, Version};

use super::set::OrderedSetRestrict;
use super::str::Restrict as StrRestrict;
use super::{Restrict as BaseRestrict, Restriction};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict {
    Category(StrRestrict),
    Package(StrRestrict),
    Blocker(Option<Blocker>),
    Version(Option<Version>),
    VersionStr(StrRestrict),
    Slot(Option<StrRestrict>),
    Subslot(Option<StrRestrict>),
    UseDeps(Option<OrderedSetRestrict<String, StrRestrict>>),
    Repo(Option<StrRestrict>),
}

impl Restrict {
    pub fn category(s: &str) -> Self {
        Self::Category(StrRestrict::equal(s))
    }

    pub fn package(s: &str) -> Self {
        Self::Package(StrRestrict::equal(s))
    }

    pub fn version(s: &str) -> crate::Result<Self> {
        let v = Version::from_str(s)?;
        Ok(Self::Version(Some(v)))
    }

    pub fn slot(o: Option<&str>) -> Self {
        Self::Slot(o.map(StrRestrict::equal))
    }

    pub fn subslot(o: Option<&str>) -> Self {
        Self::Subslot(o.map(StrRestrict::equal))
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
        Self::Repo(o.map(StrRestrict::equal))
    }
}

impl Restriction<&Dep> for Restrict {
    fn matches(&self, dep: &Dep) -> bool {
        use self::Restrict::*;
        match self {
            Category(r) => r.matches(dep.category()),
            Package(r) => r.matches(dep.package()),
            Blocker(b) => match (b, dep.blocker()) {
                (Some(b), Some(blocker)) => *b == blocker,
                (None, None) => true,
                _ => false,
            },
            Version(v) => match (v, dep.version()) {
                (Some(v), Some(ver)) => v.intersects(ver),
                (None, None) => true,
                _ => false,
            },
            VersionStr(r) => r.matches(dep.version().map_or_else(|| "", |v| v.as_str())),
            Slot(r) => match (r, dep.slot()) {
                (Some(r), Some(slot)) => r.matches(slot),
                (None, None) => true,
                _ => false,
            },
            Subslot(r) => match (r, dep.subslot()) {
                (Some(r), Some(subslot)) => r.matches(subslot),
                (None, None) => true,
                _ => false,
            },
            UseDeps(r) => match (r, dep.use_deps()) {
                (Some(r), Some(vals)) => r.matches(vals),
                (None, None) => true,
                _ => false,
            },
            Repo(r) => match (r, dep.repo()) {
                (Some(r), Some(repo)) => r.matches(repo),
                (None, None) => true,
                _ => false,
            },
        }
    }
}

impl From<Restrict> for BaseRestrict {
    fn from(r: Restrict) -> Self {
        Self::Dep(r)
    }
}

impl Restriction<&Dep> for BaseRestrict {
    fn matches(&self, dep: &Dep) -> bool {
        crate::restrict::restrict_match! {self, dep,
            Self::Dep(r) => r.matches(dep),
        }
    }
}

impl From<&Dep> for BaseRestrict {
    fn from(dep: &Dep) -> Self {
        let mut restricts = vec![
            Restrict::category(dep.category()),
            Restrict::package(dep.package()),
            Restrict::Blocker(dep.blocker()),
        ];

        if let Some(v) = dep.version() {
            restricts.push(Restrict::Version(Some(v.clone())));
        }

        if let Some(s) = dep.slot() {
            restricts.push(Restrict::slot(Some(s)));
        }

        if let Some(s) = dep.subslot() {
            restricts.push(Restrict::subslot(Some(s)));
        }

        if let Some(u) = dep.use_deps() {
            restricts.push(Restrict::use_deps(Some(u)));
        }

        if let Some(s) = dep.repo() {
            restricts.push(Restrict::repo(Some(s)));
        }

        BaseRestrict::and(restricts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restrict_methods() {
        let unversioned = Dep::from_str("cat/pkg").unwrap();
        let blocker = Dep::from_str("!cat/pkg").unwrap();
        let cpv = Dep::new_cpv("cat/pkg-1").unwrap();
        let full = Dep::from_str("=cat/pkg-1:2/3[u1,u2]::repo").unwrap();

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
        let unversioned = Dep::from_str("cat/pkg").unwrap();
        let cpv = Dep::new_cpv("cat/pkg-1").unwrap();
        let full = Dep::from_str("=cat/pkg-1:2/3[u1,u2]::repo").unwrap();

        // unversioned restriction
        let r = BaseRestrict::from(&unversioned);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // cpv restriction
        let r = BaseRestrict::from(&cpv);
        assert!(!r.matches(&unversioned));
        assert!(r.matches(&cpv));
        assert!(r.matches(&full));

        // full restriction
        let r = BaseRestrict::from(&full);
        assert!(!r.matches(&unversioned));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&full));
    }

    #[test]
    fn test_restrict_versions() {
        let lt = Dep::from_str("<cat/pkg-1-r1").unwrap();
        let le = Dep::from_str("<=cat/pkg-1-r1").unwrap();
        let eq = Dep::from_str("=cat/pkg-1-r1").unwrap();
        let eq_glob = Dep::from_str("=cat/pkg-1*").unwrap();
        let approx = Dep::from_str("~cat/pkg-1").unwrap();
        let ge = Dep::from_str(">=cat/pkg-1-r1").unwrap();
        let gt = Dep::from_str(">cat/pkg-1-r1").unwrap();

        let lt_cpv = Dep::new_cpv("cat/pkg-0").unwrap();
        let gt_cpv = Dep::new_cpv("cat/pkg-2").unwrap();

        let r = BaseRestrict::from(&lt);
        assert!(r.matches(&lt_cpv));
        assert!(r.matches(&lt));
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&le);
        assert!(r.matches(&lt_cpv));
        assert!(r.matches(&le));
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&eq);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq));
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&eq_glob);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&eq_glob));
        for s in ["cat/pkg-1-r1", "cat/pkg-10", "cat/pkg-1.0.1"] {
            let cpv = Dep::new_cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));
        let r = BaseRestrict::from(&approx);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&approx));
        for s in ["cat/pkg-1-r1", "cat/pkg-1-r999"] {
            let cpv = Dep::new_cpv(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));

        let r = BaseRestrict::from(&ge);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&ge));
        assert!(r.matches(&gt_cpv));

        let r = BaseRestrict::from(&gt);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&gt));
        assert!(r.matches(&gt_cpv));
    }
}
