use std::hash::Hash;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, Sub, SubAssign,
};
use std::sync::Arc;

use indexmap::IndexSet;

use crate::dep::{Cpn, Cpv, Dep, Version};
use crate::pkg::Pkg;
use crate::repo::ebuild::Repo as EbuildRepo;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::{Restrict, Restriction};
use crate::traits::Contains;
use crate::types::OrderedSet;

use super::{PkgRepository, Repo, Repository};

/// Ordered set of repos
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoSet(OrderedSet<Repo>);

impl RepoSet {
    /// Construct a new, empty `RepoSet`.
    pub fn new() -> Self {
        Self(OrderedSet::new())
    }

    /// Return the ordered set of all repos in the set.
    pub fn repos(&self) -> &OrderedSet<Repo> {
        &self.0
    }

    /// Iterate over all ebuild repos in the set.
    pub fn ebuild(&self) -> impl Iterator<Item = &Arc<EbuildRepo>> {
        self.0.iter().filter_map(|r| r.as_ebuild())
    }

    /// Filter a repo set using repo restrictions.
    pub fn filter(self, restrict: Restrict) -> (Self, Restrict) {
        if let Restrict::And(vals) = &restrict {
            use DepRestrict::Repo;
            let mut repo_restricts = vec![];
            let mut restricts = vec![];
            for r in vals.iter().map(Deref::deref) {
                match r {
                    Restrict::Dep(Repo(Some(r))) => repo_restricts.push(r),
                    r => restricts.push(r),
                }
            }

            if !repo_restricts.is_empty() {
                let set = self
                    .into_iter()
                    .filter(|r| repo_restricts.iter().all(|res| res.matches(r.id())))
                    .collect();
                return (set, Restrict::and(restricts));
            }
        }

        (self, restrict)
    }
}

impl Default for RepoSet {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Into<Repo>> FromIterator<R> for RepoSet {
    fn from_iter<I: IntoIterator<Item = R>>(iterable: I) -> Self {
        let mut repos: OrderedSet<_> = iterable.into_iter().map(Into::into).collect();
        repos.sort();
        Self(repos)
    }
}

impl From<Repo> for RepoSet {
    fn from(repo: Repo) -> Self {
        [repo].into_iter().collect()
    }
}

impl From<&Repo> for RepoSet {
    fn from(repo: &Repo) -> Self {
        repo.clone().into()
    }
}

impl PkgRepository for RepoSet {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type IterCpv<'a> = IterCpv where Self: 'a;
    type Iter<'a> = Iter<'a> where Self: 'a;
    type IterRestrict<'a> = Iter<'a> where Self: 'a;

    fn categories(&self) -> IndexSet<String> {
        let mut cats: IndexSet<_> = self.0.iter().flat_map(|r| r.categories()).collect();
        cats.sort();
        cats
    }

    fn packages(&self, cat: &str) -> IndexSet<String> {
        let mut pkgs: IndexSet<_> = self.0.iter().flat_map(|r| r.packages(cat)).collect();
        pkgs.sort();
        pkgs
    }

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version<String>> {
        let mut versions: IndexSet<_> = self.0.iter().flat_map(|r| r.versions(cat, pkg)).collect();
        versions.sort();
        versions
    }

    fn len(&self) -> usize {
        self.iter().count()
    }

    fn iter_cpv(&self) -> Self::IterCpv<'_> {
        let mut cpvs: IndexSet<_> = self.0.iter().flat_map(|r| r.iter_cpv()).collect();
        cpvs.sort();
        IterCpv(cpvs.into_iter())
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict<'_> {
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

        Iter(Box::new(
            self.0
                .iter()
                .filter(move |r| repo_restrict.matches(r.id()))
                .flat_map(move |r| r.iter_restrict(restrict.clone())),
        ))
    }
}

impl Contains<&Cpn<String>> for RepoSet {
    fn contains(&self, value: &Cpn<String>) -> bool {
        self.0.iter().any(|r| r.contains(value))
    }
}

impl Contains<&Cpv<String>> for RepoSet {
    fn contains(&self, value: &Cpv<String>) -> bool {
        self.0.iter().any(|r| r.contains(value))
    }
}

impl Contains<&Dep<String>> for RepoSet {
    fn contains(&self, value: &Dep<String>) -> bool {
        self.0.iter().any(|r| r.contains(value))
    }
}

pub struct IterCpv(indexmap::set::IntoIter<Cpv<String>>);

impl Iterator for IterCpv {
    type Item = Cpv<String>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl IntoIterator for RepoSet {
    type Item = Repo;
    type IntoIter = indexmap::set::IntoIter<Repo>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

// TODO: Use type alias impl trait support for IntoIterator implementation when stable in order to
// replace boxed type with a generic type.
//
// See https://github.com/rust-lang/rust/issues/63063
pub struct Iter<'a>(Box<dyn Iterator<Item = Pkg<'a>> + 'a>);

impl<'a> IntoIterator for &'a RepoSet {
    type Item = Pkg<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(Box::new(self.0.iter().flat_map(|r| r.into_iter())))
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Pkg<'a>;

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
        self.0 &= &other.0;
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
        self.0 |= &other.0;
        self.0.sort();
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
        self.0 ^= &other.0;
        self.0.sort();
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
        self.0 -= &other.0;
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
        self.0 &= &set;
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
        self.0 |= &set;
        self.0.sort();
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
        self.0 ^= &set;
        self.0.sort();
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
        self.0 -= &set;
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::RepoPackage;
    use crate::repo::{fake, Contains, Repository};
    use crate::test::assert_ordered_eq;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn test_cmp_traits() {
        let s1 = RepoSet::new();
        let s2 = RepoSet::new();
        assert_eq!(s1, s2);
        assert_eq!(hash(s1), hash(s2));

        // different parameter order are still sorted lexically by repo id
        let r1: Repo = fake::Repo::new("r1", 0).into();
        let r2: Repo = fake::Repo::new("r2", 0).into();
        let s1 = RepoSet::from_iter([&r1, &r2]);
        let s2 = RepoSet::from_iter([&r2, &r1]);
        assert_eq!(s1, s2);
        assert_eq!(hash(s1), hash(s2));

        // higher priority repos come before lower priority ones
        let r1: Repo = fake::Repo::new("r1", -1).into();
        let r2: Repo = fake::Repo::new("r2", 0).into();
        let s1 = RepoSet::from_iter([&r1]);
        let s2 = RepoSet::from_iter([&r2]);
        assert!(s2 < s1);
        assert_ne!(s1, s2);
        assert_ne!(hash(s1), hash(s2));
    }

    #[test]
    fn test_repos() {
        let s = RepoSet::new();
        assert!(s.repos().is_empty());

        let r1: Repo = fake::Repo::new("r1", 0).into();
        let r2: Repo = fake::Repo::new("r2", 0).into();
        let s = RepoSet::from_iter([&r1, &r2]);
        assert_ordered_eq(s.repos(), [&r1, &r2]);
        // different parameter order are still sorted lexically by repo id
        let s = RepoSet::from_iter([&r2, &r1]);
        assert_ordered_eq(s.repos(), [&r1, &r2]);

        // higher priority repos come before lower priority ones
        let r1: Repo = fake::Repo::new("r1", -1).into();
        let r2: Repo = fake::Repo::new("r2", 0).into();
        let s = RepoSet::from_iter([&r1, &r2]);
        assert_ordered_eq(s.repos(), [&r2, &r1]);
    }

    #[test]
    fn test_repo_traits() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let fake_repo = fake::Repo::new("fake", 0);

        let f_repo: Repo = fake_repo.into();
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();

        // empty repo set
        let s = RepoSet::new();
        assert!(s.categories().is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
        assert!(s.iter_cpv().next().is_none());
        assert!(s.iter().next().is_none());
        assert!(s.iter_restrict(&cpv).next().is_none());
        assert!(!s.contains(&cpv));

        // repo set with no pkgs
        let s = RepoSet::from_iter([&t.repo, &f_repo]);
        assert!(s.categories().is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
        assert!(s.iter_cpv().next().is_none());
        assert!(s.iter().next().is_none());
        assert!(s.iter_restrict(&cpv).next().is_none());
        assert!(!s.contains(&cpv));

        // single ebuild
        t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        assert_ordered_eq(s.categories(), ["cat"]);
        assert_ordered_eq(s.packages("cat"), ["pkg"]);
        assert_ordered_eq(s.versions("cat", "pkg"), [Version::try_new("1").unwrap()]);
        assert_eq!(s.len(), 1);
        assert!(!s.is_empty());
        assert_ordered_eq(s.iter_cpv(), [cpv.clone()]);
        assert!(s.iter().next().is_some());
        assert!(s.iter_restrict(&cpv).next().is_some());
        assert!(s.contains(&cpv));

        // multiple pkgs of different types
        let fake_repo = fake::Repo::new("fake", 0).pkgs(["cat/pkg-1"]);
        let f_repo: Repo = fake_repo.into();
        let s = RepoSet::from_iter([&t.repo, &f_repo]);
        assert_ordered_eq(s.categories(), ["cat"]);
        assert_ordered_eq(s.packages("cat"), ["pkg"]);
        assert_ordered_eq(s.versions("cat", "pkg"), [Version::try_new("1").unwrap()]);
        assert_eq!(s.len(), 2);
        assert!(s.contains(&cpv));
        assert_ordered_eq(s.iter_cpv(), [cpv.clone()]);
        assert_eq!(s.iter().count(), 2);
        assert_eq!(s.iter_restrict(&cpv).count(), 2);
        let pkg = s.iter_restrict(&cpv).next().unwrap();
        // pkg restriction only matches the repo's pkg it came from
        assert_eq!(s.iter_restrict(&pkg).count(), 1);
        assert_eq!(s.iter_restrict(&pkg).next().unwrap().repo().id(), "fake");
    }

    #[test]
    fn test_set_ops() {
        let cpv1 = Cpv::try_new("cat/pkg-1").unwrap();
        let cpv2 = Cpv::try_new("cat/pkg-2").unwrap();
        let cpv3 = Cpv::try_new("cat/pkg-3").unwrap();
        let cpv4 = Cpv::try_new("cat/pkg-4").unwrap();
        let r1: Repo = fake::Repo::new("1", 0).pkgs([&cpv1]).into();
        let r2: Repo = fake::Repo::new("2", 0).pkgs([&cpv2]).into();
        let r3: Repo = fake::Repo::new("3", 0).pkgs([&cpv3]).into();
        let r4: Repo = fake::Repo::new("4", 0).pkgs([&cpv4]).into();

        let mut s = RepoSet::new();
        assert!(!s.contains(&cpv1));

        // combine repo set and repo
        s |= &r1;
        assert!(s.contains(&cpv1));
        // combine repo set and repo set
        s |= &RepoSet::from_iter([&r2]);
        assert!(s.contains(&cpv2));
        // combine repo set and repo
        let s = s | &r3;
        assert!(s.contains(&cpv3));
        // combine repo set and repo set
        let s = s | &RepoSet::from_iter([&r4]);
        assert!(s.contains(&cpv4));

        // subtract repo set and repo set
        let s = s - &RepoSet::from_iter([&r4]);
        assert!(!s.contains(&cpv4));
        // subtract repo set and repo
        let mut s = s - &r3;
        assert!(!s.contains(&cpv3));
        // subtract repo set and repo set
        s -= &RepoSet::from_iter([&r2]);
        assert!(!s.contains(&cpv2));
        // subtract repo set and repo
        s -= &r1;
        assert!(!s.contains(&cpv1));
    }
}
