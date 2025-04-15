use std::hash::Hash;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, Sub, SubAssign,
};

use indexmap::IndexSet;

use crate::dep::{Cpn, Cpv, Dep, Version};
use crate::pkg::Pkg;
use crate::pkg::Restrict as PkgRestrict;
use crate::repo::ebuild::EbuildRepo;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::{Restrict, Restriction};
use crate::traits::Contains;
use crate::types::OrderedSet;

use super::{PkgRepository, Repo, Repository};

/// Ordered set of repos.
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoSet {
    pub repos: OrderedSet<Repo>,
}

impl RepoSet {
    /// Construct a new, empty `RepoSet`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Iterate over ebuild repos in the set.
    pub fn iter_ebuild(&self) -> impl Iterator<Item = &EbuildRepo> {
        self.repos.iter().filter_map(|r| r.as_ebuild())
    }

    /// Filter a repo set using repo restrictions.
    pub fn filter(self, restrict: Restrict) -> (Self, Restrict) {
        let mut repo_restricts = vec![];
        let mut restricts = vec![];

        // try to extract repo restrictions to perform repo filtering
        if let Restrict::And(vals) = &restrict {
            for r in vals.iter().map(Deref::deref) {
                match r {
                    Restrict::Dep(DepRestrict::Repo(Some(r))) => repo_restricts.push(r),
                    Restrict::Pkg(PkgRestrict::Repo(r)) => repo_restricts.push(r),
                    r => restricts.push(r),
                }
            }
        } else if let Restrict::Dep(DepRestrict::Repo(Some(r))) = &restrict {
            repo_restricts.push(r);
        } else if let Restrict::Pkg(PkgRestrict::Repo(r)) = &restrict {
            repo_restricts.push(r);
        }

        if !repo_restricts.is_empty() {
            let set = self
                .into_iter()
                .filter(|repo| repo_restricts.iter().all(|r| r.matches(repo.id())))
                .collect();
            (set, Restrict::and(restricts))
        } else {
            (self, restrict)
        }
    }
}

impl<R: Into<Repo>> FromIterator<R> for RepoSet {
    fn from_iter<I: IntoIterator<Item = R>>(iterable: I) -> Self {
        let mut repos: OrderedSet<_> = iterable.into_iter().map(Into::into).collect();
        repos.sort_unstable();
        Self { repos }
    }
}

impl From<Repo> for RepoSet {
    fn from(repo: Repo) -> Self {
        [repo].into_iter().collect()
    }
}

impl PkgRepository for RepoSet {
    type Pkg = Pkg;
    type IterCpn = IterCpn;
    type IterCpnRestrict = IterCpnRestrict;
    type IterCpv = IterCpv;
    type IterCpvRestrict = IterCpvRestrict;
    type Iter = Iter;
    type IterRestrict = IterRestrict;

    fn categories(&self) -> IndexSet<String> {
        let mut cats: IndexSet<_> = self.repos.iter().flat_map(|r| r.categories()).collect();
        cats.sort_unstable();
        cats
    }

    fn packages(&self, cat: &str) -> IndexSet<String> {
        let mut pkgs: IndexSet<_> = self.repos.iter().flat_map(|r| r.packages(cat)).collect();
        pkgs.sort_unstable();
        pkgs
    }

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        let mut versions: IndexSet<_> = self
            .repos
            .iter()
            .flat_map(|r| r.versions(cat, pkg))
            .collect();
        versions.sort_unstable();
        versions
    }

    fn len(&self) -> usize {
        self.iter().count()
    }

    fn iter_cpn(&self) -> Self::IterCpn {
        let mut cpns: IndexSet<_> = self.repos.iter().flat_map(|r| r.iter_cpn()).collect();
        cpns.sort_unstable();
        IterCpn(cpns.into_iter())
    }

    fn iter_cpn_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpnRestrict {
        IterCpnRestrict {
            iter: self.iter_cpn(),
            restrict: value.into(),
        }
    }

    fn iter_cpv(&self) -> Self::IterCpv {
        let mut cpvs: IndexSet<_> = self.repos.iter().flat_map(|r| r.iter_cpv()).collect();
        cpvs.sort_unstable();
        IterCpv(cpvs.into_iter())
    }

    fn iter_cpv_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpvRestrict {
        IterCpvRestrict {
            iter: self.iter_cpv(),
            restrict: value.into(),
        }
    }

    fn iter(&self) -> Self::Iter {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict {
        let restrict = val.into();

        // extract repo restrictions for filtering
        use crate::pkg::Restrict::Repo as PkgRepo;
        use DepRestrict::Repo as DepRepo;
        let mut repo_restricts = vec![];

        if let Restrict::And(vals) = &restrict {
            for r in vals.iter().map(Deref::deref) {
                match r {
                    Restrict::Dep(DepRepo(Some(x))) => repo_restricts.push(x.clone()),
                    Restrict::Pkg(PkgRepo(x)) => repo_restricts.push(x.clone()),
                    _ => (),
                }
            }
        } else if let Restrict::Pkg(PkgRepo(x)) = &restrict {
            repo_restricts.push(x.clone());
        }

        let repo_restrict = match &repo_restricts[..] {
            [] => Restrict::True,
            [_] => repo_restricts.remove(0).into(),
            _ => Restrict::and(repo_restricts),
        };

        IterRestrict(Box::new(
            self.repos
                .clone()
                .into_iter()
                .filter(move |r| repo_restrict.matches(r.id()))
                .flat_map(move |r| r.iter_restrict(restrict.clone())),
        ))
    }
}

impl Contains<&Repo> for RepoSet {
    fn contains(&self, value: &Repo) -> bool {
        self.repos.contains(value)
    }
}

impl Contains<&str> for RepoSet {
    fn contains(&self, value: &str) -> bool {
        self.repos.iter().any(|r| r.id() == value)
    }
}

impl Contains<&Cpn> for RepoSet {
    fn contains(&self, value: &Cpn) -> bool {
        self.repos.iter().any(|r| r.contains(value))
    }
}

impl Contains<&Cpv> for RepoSet {
    fn contains(&self, value: &Cpv) -> bool {
        self.repos.iter().any(|r| r.contains(value))
    }
}

impl Contains<&Dep> for RepoSet {
    fn contains(&self, value: &Dep) -> bool {
        self.repos.iter().any(|r| r.contains(value))
    }
}

impl Contains<&Restrict> for RepoSet {
    fn contains(&self, value: &Restrict) -> bool {
        self.repos.iter().any(|r| r.contains(value))
    }
}

pub struct IterCpn(indexmap::set::IntoIter<Cpn>);

impl Iterator for IterCpn {
    type Item = Cpn;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

pub struct IterCpnRestrict {
    iter: IterCpn,
    restrict: Restrict,
}

impl Iterator for IterCpnRestrict {
    type Item = Cpn;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|cpn| self.restrict.matches(cpn))
    }
}

pub struct IterCpv(indexmap::set::IntoIter<Cpv>);

impl Iterator for IterCpv {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

pub struct IterCpvRestrict {
    iter: IterCpv,
    restrict: Restrict,
}

impl Iterator for IterCpvRestrict {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|cpv| self.restrict.matches(cpv))
    }
}

impl IntoIterator for RepoSet {
    type Item = Repo;
    type IntoIter = indexmap::set::IntoIter<Repo>;

    fn into_iter(self) -> Self::IntoIter {
        self.repos.into_iter()
    }
}

// TODO: Use type alias impl trait support for IntoIterator implementation when stable in order to
// replace boxed type with a generic type.
//
// See https://github.com/rust-lang/rust/issues/63063
pub struct Iter(Box<dyn Iterator<Item = crate::Result<Pkg>>>);

impl IntoIterator for &RepoSet {
    type Item = crate::Result<Pkg>;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        Iter(Box::new(self.repos.clone().into_iter().flat_map(|r| r.into_iter())))
    }
}

impl Iterator for Iter {
    type Item = crate::Result<Pkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

// TODO: Use type alias impl trait support for IntoIterator implementation when stable in order to
// replace boxed type with a generic type.
//
// See https://github.com/rust-lang/rust/issues/63063
pub struct IterRestrict(Box<dyn Iterator<Item = crate::Result<Pkg>>>);

impl Iterator for IterRestrict {
    type Item = crate::Result<Pkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl BitAnd<&Self> for RepoSet {
    type Output = Self;

    fn bitand(mut self, other: &Self) -> Self::Output {
        self &= other;
        self
    }
}

impl BitAndAssign<&Self> for RepoSet {
    fn bitand_assign(&mut self, other: &Self) {
        self.repos &= &other.repos;
    }
}

impl BitOr<&Self> for RepoSet {
    type Output = Self;

    fn bitor(mut self, other: &Self) -> Self::Output {
        self |= other;
        self
    }
}

impl BitOrAssign<&Self> for RepoSet {
    fn bitor_assign(&mut self, other: &Self) {
        self.repos |= &other.repos;
        self.repos.sort_unstable();
    }
}

impl BitXor<&Self> for RepoSet {
    type Output = Self;

    fn bitxor(mut self, other: &Self) -> Self::Output {
        self ^= other;
        self
    }
}

impl BitXorAssign<&Self> for RepoSet {
    fn bitxor_assign(&mut self, other: &Self) {
        self.repos ^= &other.repos;
        self.repos.sort_unstable();
    }
}

impl Sub<&Self> for RepoSet {
    type Output = Self;

    fn sub(mut self, other: &Self) -> Self::Output {
        self -= other;
        self
    }
}

impl SubAssign<&Self> for RepoSet {
    fn sub_assign(&mut self, other: &Self) {
        self.repos -= &other.repos;
    }
}

impl BitAnd<&Repo> for RepoSet {
    type Output = Self;

    fn bitand(mut self, other: &Repo) -> Self::Output {
        self &= other;
        self
    }
}

impl BitAndAssign<&Repo> for RepoSet {
    fn bitand_assign(&mut self, other: &Repo) {
        let set = [other.clone()].into_iter().collect();
        self.repos &= &set;
    }
}

impl BitOr<&Repo> for RepoSet {
    type Output = Self;

    fn bitor(mut self, other: &Repo) -> Self::Output {
        self |= other;
        self
    }
}

impl BitOrAssign<&Repo> for RepoSet {
    fn bitor_assign(&mut self, other: &Repo) {
        let set = [other.clone()].into_iter().collect();
        self.repos |= &set;
        self.repos.sort_unstable();
    }
}

impl BitXor<&Repo> for RepoSet {
    type Output = Self;

    fn bitxor(mut self, other: &Repo) -> Self::Output {
        self ^= other;
        self
    }
}

impl BitXorAssign<&Repo> for RepoSet {
    fn bitxor_assign(&mut self, other: &Repo) {
        let set = [other.clone()].into_iter().collect();
        self.repos ^= &set;
        self.repos.sort_unstable();
    }
}

impl Sub<&Repo> for RepoSet {
    type Output = Self;

    fn sub(mut self, other: &Repo) -> Self::Output {
        self -= other;
        self
    }
}

impl SubAssign<&Repo> for RepoSet {
    fn sub_assign(&mut self, other: &Repo) {
        let set = [other.clone()].into_iter().collect();
        self.repos -= &set;
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::RepoPackage;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::repo::fake::FakeRepo;
    use crate::test::*;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn cmp() {
        let s1 = RepoSet::new();
        let s2 = RepoSet::new();
        assert_eq!(s1, s2);
        assert_eq!(hash(s1), hash(s2));

        // different parameter order are still sorted lexically by repo id
        let r1: Repo = FakeRepo::new("r1", 0).into();
        let r2: Repo = FakeRepo::new("r2", 0).into();
        let s1 = RepoSet::from_iter([&r1, &r2]);
        let s2 = RepoSet::from_iter([&r2, &r1]);
        assert_eq!(s1, s2);
        assert_eq!(hash(s1), hash(s2));

        // higher priority repos come before lower priority ones
        let r1: Repo = FakeRepo::new("r1", -1).into();
        let r2: Repo = FakeRepo::new("r2", 0).into();
        let s1 = RepoSet::from_iter([&r1]);
        let s2 = RepoSet::from_iter([&r2]);
        assert!(s2 < s1);
        assert_ne!(s1, s2);
        assert_ne!(hash(s1), hash(s2));
    }

    #[test]
    fn repos() {
        let s = RepoSet::new();
        assert!(s.repos.is_empty());

        let r1: Repo = FakeRepo::new("r1", 0).into();
        let r2: Repo = FakeRepo::new("r2", 0).into();
        let s = RepoSet::from_iter([&r1, &r2]);
        assert_ordered_eq!(&s.repos, [&r1, &r2]);
        // different parameter order are still sorted lexically by repo id
        let s = RepoSet::from_iter([&r2, &r1]);
        assert_ordered_eq!(&s.repos, [&r1, &r2]);

        // higher priority repos come before lower priority ones
        let r1: Repo = FakeRepo::new("r1", -1).into();
        let r2: Repo = FakeRepo::new("r2", 0).into();
        let s = RepoSet::from_iter([&r1, &r2]);
        assert_ordered_eq!(&s.repos, [&r2, &r1]);
    }

    #[test]
    fn filter() {
        let r1: Repo = FakeRepo::new("r1", 0).into();
        let r2: Repo = FakeRepo::new("r2", 0).into();
        let s = RepoSet::from_iter([&r1, &r2]);
        let s1 = RepoSet::from_iter([&r1]);
        let s2 = RepoSet::from_iter([&r2]);

        let restrict: Restrict = DepRestrict::repo(Some("r1")).into();
        let (new, _) = s.clone().filter(restrict);
        assert_eq!(new, s1);
        let restrict: Restrict = PkgRestrict::repo("r2").into();
        let (new, _) = s.clone().filter(restrict);
        assert_eq!(new, s2);
    }

    #[test]
    fn repo_traits() {
        let data = test_data();
        let e_repo = data.repo("empty").unwrap();
        let f_repo: Repo = FakeRepo::new("fake", 0).into();

        let cpn = Cpn::try_new("cat/pkg").unwrap();
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        let dep = Dep::try_new("=cat/pkg-1").unwrap();
        let restrict: Restrict = DepRestrict::category("cat").into();

        // empty repo set
        let s = RepoSet::new();
        assert!(s.categories().is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
        assert!(s.iter_cpn().next().is_none());
        assert!(s.iter_cpn_restrict(&restrict).next().is_none());
        assert!(s.iter_cpv().next().is_none());
        assert!(s.iter_cpv_restrict(&restrict).next().is_none());
        assert!(s.iter().next().is_none());
        assert!(s.iter_restrict(&cpv).next().is_none());
        assert!(!s.contains(&cpn));
        assert!(!s.contains(&cpv));
        assert!(!s.contains(&dep));
        assert!(!s.contains(e_repo));
        assert!(!s.contains("empty"));

        // repo set with no pkgs
        let s = RepoSet::from_iter([e_repo.clone(), f_repo.clone()]);
        assert!(s.categories().is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
        assert!(s.iter_cpn().next().is_none());
        assert!(s.iter_cpn_restrict(&restrict).next().is_none());
        assert!(s.iter_cpv().next().is_none());
        assert!(s.iter_cpv_restrict(&restrict).next().is_none());
        assert!(s.iter().next().is_none());
        assert!(s.iter_restrict(&cpv).next().is_none());
        assert!(!s.contains(&cpn));
        assert!(!s.contains(&cpv));
        assert!(!s.contains(&dep));
        assert!(s.contains(e_repo));
        assert!(s.contains("empty"));
        assert!(s.contains(&f_repo));
        assert!(s.contains("fake"));
        assert!(!s.contains("nonexistent"));

        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let e_repo = config.add_repo(&temp, false).unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        config.finalize().unwrap();

        // single ebuild
        let s = RepoSet::from_iter([e_repo.clone(), f_repo]);
        assert_ordered_eq!(s.categories(), ["cat"]);
        assert_ordered_eq!(s.packages("cat"), ["pkg"]);
        assert_ordered_eq!(s.versions("cat", "pkg"), [Version::try_new("1").unwrap()]);
        assert_eq!(s.len(), 1);
        assert!(!s.is_empty());
        assert_ordered_eq!(s.iter_cpn(), [cpn.clone()]);
        assert_ordered_eq!(s.iter_cpn_restrict(&restrict), [cpn.clone()]);
        assert_ordered_eq!(s.iter_cpv(), [cpv.clone()]);
        assert_ordered_eq!(s.iter_cpv_restrict(&restrict), [cpv.clone()]);
        assert!(s.iter().next().is_some());
        assert!(s.iter_restrict(&cpv).next().is_some());
        assert!(s.contains(&cpn));
        assert!(s.contains(&cpv));
        assert!(s.contains(&dep));

        // multiple pkgs of different types
        let f_repo: Repo = FakeRepo::new("fake", 0).pkgs(["cat/pkg-1"]).unwrap().into();
        let s = RepoSet::from_iter([e_repo, f_repo]);
        assert_ordered_eq!(s.categories(), ["cat"]);
        assert_ordered_eq!(s.packages("cat"), ["pkg"]);
        assert_ordered_eq!(s.versions("cat", "pkg"), [Version::try_new("1").unwrap()]);
        assert_eq!(s.len(), 2);
        assert!(s.contains(&cpn));
        assert!(s.contains(&cpv));
        assert!(s.contains(&dep));
        assert_ordered_eq!(s.iter_cpn(), [cpn.clone()]);
        assert_ordered_eq!(s.iter_cpn_restrict(&restrict), [cpn.clone()]);
        assert_ordered_eq!(s.iter_cpv(), [cpv.clone()]);
        assert_ordered_eq!(s.iter_cpv_restrict(&restrict), [cpv.clone()]);
        assert_eq!(s.iter().count(), 2);
        assert_eq!(s.iter_restrict(&cpv).count(), 2);
        let pkg = s.iter_restrict(&cpv).next().unwrap().unwrap();
        // pkg restriction only matches the repo's pkg it came from
        assert_eq!(s.iter_restrict(&pkg).count(), 1);
        assert_eq!(s.iter_restrict(&pkg).next().unwrap().unwrap().repo().id(), "fake");
    }

    #[test]
    fn set_ops() {
        let cpv1 = Cpv::try_new("cat/pkg-1").unwrap();
        let cpv2 = Cpv::try_new("cat/pkg-2").unwrap();
        let cpv3 = Cpv::try_new("cat/pkg-3").unwrap();
        let cpv4 = Cpv::try_new("cat/pkg-4").unwrap();
        let r1: Repo = FakeRepo::new("1", 0).pkgs([&cpv1]).unwrap().into();
        let r2: Repo = FakeRepo::new("2", 0).pkgs([&cpv2]).unwrap().into();
        let r3: Repo = FakeRepo::new("3", 0).pkgs([&cpv3]).unwrap().into();
        let r4: Repo = FakeRepo::new("4", 0).pkgs([&cpv4]).unwrap().into();

        // intersection
        // repo set and repo
        let mut s = RepoSet::from_iter([&r1, &r2]);
        s &= &r1;
        assert!(s.contains(&cpv1));
        // repo set and repo set
        let mut s = RepoSet::from_iter([&r1, &r2]);
        s &= &RepoSet::from_iter([&r2]);
        assert!(s.contains(&cpv2));
        // repo set and repo
        let s = RepoSet::from_iter([&r3]) & &r3;
        assert!(s.contains(&cpv3));
        // repo set and repo set
        let s = RepoSet::from_iter([&r1, &r4]) & &RepoSet::from_iter([&r4]);
        assert!(s.contains(&cpv4));

        // union
        let mut s = RepoSet::new();
        // repo set and repo
        s |= &r1;
        assert!(s.contains(&cpv1));
        // repo set and repo set
        s |= &RepoSet::from_iter([&r2]);
        assert!(s.contains(&cpv2));
        // repo set and repo
        let s = s | &r3;
        assert!(s.contains(&cpv3));
        // repo set and repo set
        let s = s | &RepoSet::from_iter([&r4]);
        assert!(s.contains(&cpv4));

        // difference
        // repo set and repo set
        let s = s - &RepoSet::from_iter([&r4]);
        assert!(!s.contains(&cpv4));
        // repo set and repo
        let mut s = s - &r3;
        assert!(!s.contains(&cpv3));
        // repo set and repo set
        s -= &RepoSet::from_iter([&r2]);
        assert!(!s.contains(&cpv2));
        // repo set and repo
        s -= &r1;
        assert!(!s.contains(&cpv1));

        // exclusive or
        // repo set and repo
        let mut s = RepoSet::from_iter([&r1, &r2]);
        s ^= &r1;
        assert!(!s.contains(&cpv1));
        assert!(s.contains(&cpv2));
        // repo set and repo set
        let mut s = RepoSet::from_iter([&r1, &r2]);
        s ^= &RepoSet::from_iter([&r2]);
        assert!(s.contains(&cpv1));
        assert!(!s.contains(&cpv2));
        // repo set and repo
        let s = RepoSet::from_iter([&r3]) ^ &r3;
        assert!(!s.contains(&cpv3));
        // repo set and repo set
        let s = RepoSet::from_iter([&r1, &r4]) ^ &RepoSet::from_iter([&r4]);
        assert!(s.contains(&cpv1));
        assert!(!s.contains(&cpv4));
    }
}
