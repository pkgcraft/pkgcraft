use crate::dep::{Blocker, Cpv, Dep, UseDep, Version};
use crate::traits::Intersects;
use crate::types::SortedSet;

use super::str::Restrict as StrRestrict;
use super::{Restrict as BaseRestrict, Restriction};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Restrict {
    Category(StrRestrict),
    Package(StrRestrict),
    Blocker(Option<Blocker>),
    Version(Option<Version>),
    Slot(Option<StrRestrict>),
    Subslot(Option<StrRestrict>),
    UseDeps(Option<SortedSet<UseDep<String>>>),
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
        Ok(Self::Version(Some(s.parse()?)))
    }

    pub fn slot(o: Option<&str>) -> Self {
        Self::Slot(o.map(StrRestrict::equal))
    }

    pub fn subslot(o: Option<&str>) -> Self {
        Self::Subslot(o.map(StrRestrict::equal))
    }

    pub fn use_deps(s: &str) -> crate::Result<Self> {
        Ok(Self::UseDeps(Some(s.parse()?)))
    }

    pub fn repo(o: Option<&str>) -> Self {
        Self::Repo(o.map(StrRestrict::equal))
    }
}

impl Restriction<&Cpv> for Restrict {
    fn matches(&self, cpv: &Cpv) -> bool {
        use self::Restrict::*;
        match self {
            Category(r) => r.matches(cpv.category()),
            Package(r) => r.matches(cpv.package()),
            Version(Some(v)) => v.intersects(cpv.version()),
            Blocker(None) => true,
            Slot(None) => true,
            Subslot(None) => true,
            UseDeps(None) => true,
            Repo(None) => true,
            _ => false,
        }
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
            UseDeps(u) => match (u, dep.use_deps()) {
                (Some(u), Some(use_deps)) => u.is_subset(use_deps),
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

impl Restriction<&Cpv> for BaseRestrict {
    fn matches(&self, cpv: &Cpv) -> bool {
        crate::restrict::restrict_match! {self, cpv,
            Self::Dep(r) => r.matches(cpv),
        }
    }
}

impl Restriction<&Dep> for BaseRestrict {
    fn matches(&self, dep: &Dep) -> bool {
        crate::restrict::restrict_match! {self, dep,
            Self::Dep(r) => r.matches(dep),
        }
    }
}

impl From<&Cpv> for BaseRestrict {
    fn from(cpv: &Cpv) -> Self {
        BaseRestrict::and([
            Restrict::category(cpv.category()),
            Restrict::package(cpv.package()),
            Restrict::Version(Some(cpv.version().clone())),
        ])
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
            restricts.push(Restrict::UseDeps(Some(u.clone())));
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
        let unversioned = Dep::new("cat/pkg").unwrap();
        let blocker = Dep::new("!cat/pkg").unwrap();
        let cpv = Cpv::new("cat/pkg-1").unwrap();
        let full = Dep::new("=cat/pkg-1:2/3::repo[u1,u2]").unwrap();

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
        let r = Restrict::UseDeps(None);
        assert!(r.matches(&unversioned));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&full));

        // use deps specified
        for s in ["u1", "u1,u2"] {
            let r = Restrict::use_deps(s).unwrap();
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
        let unversioned = Dep::new("cat/pkg").unwrap();
        let cpv = Cpv::new("cat/pkg-1").unwrap();
        let full = Dep::new("=cat/pkg-1:2/3::repo[u1,u2]").unwrap();

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
        let lt = Dep::new("<cat/pkg-1-r1").unwrap();
        let le = Dep::new("<=cat/pkg-1-r1").unwrap();
        let eq = Dep::new("=cat/pkg-1-r1").unwrap();
        let eq_glob = Dep::new("=cat/pkg-1*").unwrap();
        let approx = Dep::new("~cat/pkg-1").unwrap();
        let ge = Dep::new(">=cat/pkg-1-r1").unwrap();
        let gt = Dep::new(">cat/pkg-1-r1").unwrap();

        let lt_cpv = Cpv::new("cat/pkg-0").unwrap();
        let gt_cpv = Cpv::new("cat/pkg-2").unwrap();

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
            let cpv = Cpv::new(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));
        let r = BaseRestrict::from(&approx);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&approx));
        for s in ["cat/pkg-1-r1", "cat/pkg-1-r999"] {
            let cpv = Cpv::new(s).unwrap();
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
