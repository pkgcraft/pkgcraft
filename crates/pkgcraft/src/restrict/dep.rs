use std::borrow::Cow;

use crate::dep::{Blocker, Cpn, Cpv, Dep, UseDep, Version};
use crate::pkg::{Package, Pkg, RepoPackage};
use crate::repo::{Repo, Repository};
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
    UseDeps(Option<SortedSet<UseDep>>),
    Repo(Option<StrRestrict>),
}

impl Restrict {
    pub fn category<S: Into<String>>(s: S) -> Self {
        Self::Category(StrRestrict::equal(s))
    }

    pub fn package<S: Into<String>>(s: S) -> Self {
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

impl Restriction<&Cpn> for Restrict {
    fn matches(&self, cpn: &Cpn) -> bool {
        match self {
            Self::Category(r) => r.matches(cpn.category()),
            Self::Package(r) => r.matches(cpn.package()),
            Self::Version(None) => true,
            Self::Blocker(None) => true,
            Self::Slot(None) => true,
            Self::Subslot(None) => true,
            Self::UseDeps(None) => true,
            Self::Repo(None) => true,
            _ => false,
        }
    }
}

impl Restriction<&Cpv> for Restrict {
    fn matches(&self, cpv: &Cpv) -> bool {
        match self {
            Self::Category(r) => r.matches(cpv.category()),
            Self::Package(r) => r.matches(cpv.package()),
            Self::Version(Some(v)) => v.intersects(cpv.version()),
            Self::Blocker(None) => true,
            Self::Slot(None) => true,
            Self::Subslot(None) => true,
            Self::UseDeps(None) => true,
            Self::Repo(None) => true,
            _ => false,
        }
    }
}

impl Restriction<&Dep> for Restrict {
    fn matches(&self, dep: &Dep) -> bool {
        match self {
            Self::Category(r) => r.matches(dep.category()),
            Self::Package(r) => r.matches(dep.package()),
            Self::Blocker(b) => match (b, dep.blocker()) {
                (Some(b), Some(blocker)) => *b == blocker,
                (None, None) => true,
                _ => false,
            },
            Self::Version(v) => match (v, dep.version()) {
                (Some(v), Some(ver)) => v.intersects(ver),
                (None, None) => true,
                _ => false,
            },
            Self::Slot(r) => match (r, dep.slot()) {
                (Some(r), Some(slot)) => r.matches(slot),
                (None, None) => true,
                _ => false,
            },
            Self::Subslot(r) => match (r, dep.subslot()) {
                (Some(r), Some(subslot)) => r.matches(subslot),
                (None, None) => true,
                _ => false,
            },
            Self::UseDeps(u) => match (u, dep.use_deps()) {
                (Some(u), Some(use_deps)) => u.is_subset(use_deps),
                (None, None) => true,
                _ => false,
            },
            Self::Repo(r) => match (r, dep.repo()) {
                (Some(r), Some(repo)) => r.matches(repo),
                (None, None) => true,
                _ => false,
            },
        }
    }
}

impl Restriction<&str> for Restrict {
    fn matches(&self, s: &str) -> bool {
        match self {
            Self::Category(r) => r.matches(s),
            Self::Package(r) => r.matches(s),
            Self::Slot(Some(r)) => r.matches(s),
            Self::Subslot(Some(r)) => r.matches(s),
            Self::Repo(Some(r)) => r.matches(s),
            _ => false,
        }
    }
}

impl Restriction<&Cow<'_, Dep>> for Restrict {
    fn matches(&self, value: &Cow<'_, Dep>) -> bool {
        self.matches(value.as_ref())
    }
}

impl Restriction<&Pkg> for Restrict {
    fn matches(&self, pkg: &Pkg) -> bool {
        match self {
            Self::Repo(Some(r)) => r.matches(pkg.repo().id()),
            r => r.matches(pkg.cpv()),
        }
    }
}

impl Restriction<&Repo> for Restrict {
    fn matches(&self, repo: &Repo) -> bool {
        match self {
            Self::Repo(Some(r)) => r.matches(repo.id()),
            _ => false,
        }
    }
}

impl From<Restrict> for BaseRestrict {
    fn from(r: Restrict) -> Self {
        Self::Dep(r)
    }
}

impl Restriction<&Cpn> for BaseRestrict {
    fn matches(&self, cpn: &Cpn) -> bool {
        crate::restrict::restrict_match! {self, cpn,
            Self::Dep(r) => r.matches(cpn),
        }
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

impl Restriction<&Cow<'_, Dep>> for BaseRestrict {
    fn matches(&self, dep: &Cow<'_, Dep>) -> bool {
        crate::restrict::restrict_match! {self, dep,
            Self::Dep(r) => r.matches(dep.as_ref()),
        }
    }
}

impl From<Cpn> for BaseRestrict {
    fn from(cpn: Cpn) -> Self {
        BaseRestrict::and([Restrict::category(cpn.category), Restrict::package(cpn.package)])
    }
}

impl From<&Cpn> for BaseRestrict {
    fn from(cpn: &Cpn) -> Self {
        BaseRestrict::and([
            Restrict::category(cpn.category()),
            Restrict::package(cpn.package()),
        ])
    }
}

impl From<Cpv> for BaseRestrict {
    fn from(cpv: Cpv) -> Self {
        BaseRestrict::and([
            Restrict::category(cpv.cpn.category),
            Restrict::package(cpv.cpn.package),
            Restrict::Version(Some(cpv.version)),
        ])
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

impl From<Cow<'_, Dep>> for BaseRestrict {
    fn from(value: Cow<'_, Dep>) -> Self {
        value.as_ref().into()
    }
}

impl From<&Cow<'_, Dep>> for BaseRestrict {
    fn from(value: &Cow<'_, Dep>) -> Self {
        value.as_ref().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods() {
        let cpn = Cpn::try_new("cat/pkg").unwrap();
        let blocker = Dep::try_new("!cat/pkg").unwrap();
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        let dep = Dep::try_new("=cat/pkg-1:2/3::repo[u1,u2]").unwrap();
        let cow = dep.no_use_deps();

        // category
        let r = Restrict::category("cat");
        assert!(r.matches(&cpn));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));

        // package
        let r = Restrict::package("pkg");
        assert!(r.matches(&cpn));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));

        // blocker
        let r = Restrict::Blocker(None);
        assert!(r.matches(&cpn));
        assert!(!r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));
        let r = Restrict::Blocker(Some(Blocker::Weak));
        assert!(!r.matches(&cpn));
        assert!(r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&dep));
        assert!(!r.matches(&cow));

        // no version
        let r = Restrict::Version(None);
        assert!(r.matches(&cpn));
        assert!(r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(!r.matches(&dep));
        assert!(!r.matches(&cow));

        // version
        let r = Restrict::version("1").unwrap();
        assert!(!r.matches(&cpn));
        assert!(!r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));

        // no slot
        let r = Restrict::slot(None);
        assert!(r.matches(&cpn));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&dep));
        assert!(!r.matches(&cow));

        // slot
        let r = Restrict::slot(Some("2"));
        assert!(!r.matches(&cpn));
        assert!(!r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));

        // no subslot
        let r = Restrict::subslot(None);
        assert!(r.matches(&cpn));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&dep));
        assert!(!r.matches(&cow));

        // subslot
        let r = Restrict::subslot(Some("3"));
        assert!(!r.matches(&cpn));
        assert!(!r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));

        // no use deps specified
        let r = Restrict::UseDeps(None);
        assert!(r.matches(&cpn));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&dep));
        assert!(r.matches(&cow));

        // use deps specified
        for s in ["u1", "u1,u2"] {
            let r = Restrict::use_deps(s).unwrap();
            assert!(!r.matches(&cpn));
            assert!(!r.matches(&blocker));
            assert!(!r.matches(&cpv));
            assert!(r.matches(&dep));
            assert!(!r.matches(&cow));
        }

        // no repo
        let r = Restrict::repo(None);
        assert!(r.matches(&cpn));
        assert!(r.matches(&blocker));
        assert!(r.matches(&cpv));
        assert!(!r.matches(&dep));
        assert!(!r.matches(&cow));

        // repo
        let r = Restrict::repo(Some("repo"));
        assert!(!r.matches(&cpn));
        assert!(!r.matches(&blocker));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));
    }

    #[test]
    fn conversion() {
        let cpn = Cpn::try_new("cat/pkg").unwrap();
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        let dep = Dep::try_new("=cat/pkg-1:2/3::repo[u1,u2]").unwrap();
        let cow = dep.no_use_deps();

        // cpn restriction
        let r = BaseRestrict::from(&cpn);
        assert!(r.matches(&cpn));
        assert!(r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));

        // cpv restriction
        let r = BaseRestrict::from(&cpv);
        assert!(!r.matches(&cpn));
        assert!(r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));

        // dep restriction
        let r = BaseRestrict::from(&dep);
        assert!(!r.matches(&cpn));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(!r.matches(&cow));

        // cow restriction
        let r = BaseRestrict::from(&cow);
        assert!(!r.matches(&cpn));
        assert!(!r.matches(&cpv));
        assert!(r.matches(&dep));
        assert!(r.matches(&cow));
    }

    #[test]
    fn versions() {
        let lt = Dep::try_new("<cat/pkg-1-r1").unwrap();
        let le = Dep::try_new("<=cat/pkg-1-r1").unwrap();
        let eq = Dep::try_new("=cat/pkg-1-r1").unwrap();
        let eq_glob = Dep::try_new("=cat/pkg-1*").unwrap();
        let approx = Dep::try_new("~cat/pkg-1").unwrap();
        let ge = Dep::try_new(">=cat/pkg-1-r1").unwrap();
        let gt = Dep::try_new(">cat/pkg-1-r1").unwrap();

        let lt_cpv = Cpv::try_new("cat/pkg-0").unwrap();
        let gt_cpv = Cpv::try_new("cat/pkg-2").unwrap();

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
            let cpv = Cpv::try_new(s).unwrap();
            assert!(r.matches(&cpv));
        }
        assert!(!r.matches(&gt_cpv));
        let r = BaseRestrict::from(&approx);
        assert!(!r.matches(&lt_cpv));
        assert!(r.matches(&approx));
        for s in ["cat/pkg-1-r1", "cat/pkg-1-r999"] {
            let cpv = Cpv::try_new(s).unwrap();
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
