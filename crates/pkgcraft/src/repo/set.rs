use std::collections::HashSet;
use std::hash::Hash;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, Sub, SubAssign,
};

use indexmap::IndexSet;

use crate::dep::Version;
use crate::pkg::Pkg;
use crate::repo::{PkgRepository, Repo, Repository};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::{Restrict, Restriction};
use crate::types::OrderedSet;

use super::make_contains_dep;

/// Ordered set of repos
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoSet {
    repos: OrderedSet<Repo>,
}

impl RepoSet {
    pub fn new<'a, I: IntoIterator<Item = &'a Repo>>(repos: I) -> Self {
        let mut repos: OrderedSet<_> = repos.into_iter().cloned().collect();
        repos.sort();
        Self { repos }
    }

    pub fn repos(&self) -> &OrderedSet<Repo> {
        &self.repos
    }
}

impl PkgRepository for RepoSet {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type Iter<'a> = Iter<'a> where Self: 'a;
    type IterRestrict<'a> = Iter<'a> where Self: 'a;

    fn categories(&self) -> Vec<String> {
        let cats: HashSet<_> = self.repos.iter().flat_map(|r| r.categories()).collect();
        let mut cats: Vec<_> = cats.into_iter().collect();
        cats.sort();
        cats
    }

    fn packages(&self, cat: &str) -> Vec<String> {
        let pkgs: HashSet<_> = self.repos.iter().flat_map(|r| r.packages(cat)).collect();
        let mut pkgs: Vec<_> = pkgs.into_iter().collect();
        pkgs.sort();
        pkgs
    }

    fn versions(&self, cat: &str, pkg: &str) -> Vec<Version> {
        let versions: HashSet<_> = self
            .repos
            .iter()
            .flat_map(|r| r.versions(cat, pkg))
            .collect();
        let mut versions: Vec<_> = versions.into_iter().collect();
        versions.sort();
        versions
    }

    fn len(&self) -> usize {
        self.iter().count()
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
            self.repos
                .iter()
                .filter(move |r| repo_restrict.matches(r.id()))
                .flat_map(move |r| r.iter_restrict(restrict.clone())),
        ))
    }
}

make_contains_dep!(RepoSet);

// TODO: Use type alias impl trait support for IntoIterator implementation when stable in order to
// replace boxed type with a generic type.
//
// See https://github.com/rust-lang/rust/issues/63063
pub struct Iter<'a>(Box<dyn Iterator<Item = Pkg<'a>> + 'a>);

impl<'a> IntoIterator for &'a RepoSet {
    type Item = Pkg<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(Box::new(self.repos.iter().flat_map(|r| r.into_iter())))
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

    fn bitand(mut self, other: &Self) -> Self {
        self &= other;
        self
    }
}

impl BitAndAssign<&Self> for RepoSet {
    fn bitand_assign(&mut self, other: &Self) {
        self.repos = OrderedSet(self.repos.bitand(&other.repos));
        self.repos.sort();
    }
}

impl BitOr<&Self> for RepoSet {
    type Output = Self;

    fn bitor(mut self, other: &Self) -> Self {
        self |= other;
        self
    }
}

impl BitOrAssign<&Self> for RepoSet {
    fn bitor_assign(&mut self, other: &Self) {
        self.repos = OrderedSet(self.repos.bitor(&other.repos));
        self.repos.sort();
    }
}

impl BitXor<&Self> for RepoSet {
    type Output = Self;

    fn bitxor(mut self, other: &Self) -> Self {
        self ^= other;
        self
    }
}

impl BitXorAssign<&Self> for RepoSet {
    fn bitxor_assign(&mut self, other: &Self) {
        self.repos = OrderedSet(self.repos.bitxor(&other.repos));
        self.repos.sort();
    }
}

impl Sub<&Self> for RepoSet {
    type Output = Self;

    fn sub(mut self, other: &Self) -> Self {
        self -= other;
        self
    }
}

impl SubAssign<&Self> for RepoSet {
    fn sub_assign(&mut self, other: &Self) {
        self.repos = OrderedSet(self.repos.sub(&other.repos));
    }
}

impl BitAnd<&Repo> for RepoSet {
    type Output = Self;

    fn bitand(mut self, other: &Repo) -> Self {
        self &= other;
        self
    }
}

impl BitAndAssign<&Repo> for RepoSet {
    fn bitand_assign(&mut self, other: &Repo) {
        self.repos = OrderedSet(self.repos.bitand(&IndexSet::from([other.clone()])));
    }
}

impl BitOr<&Repo> for RepoSet {
    type Output = Self;

    fn bitor(mut self, other: &Repo) -> Self {
        self |= other;
        self
    }
}

impl BitOrAssign<&Repo> for RepoSet {
    fn bitor_assign(&mut self, other: &Repo) {
        if self.repos.insert(other.clone()) {
            self.repos.sort();
        }
    }
}

impl BitXor<&Repo> for RepoSet {
    type Output = Self;

    fn bitxor(mut self, other: &Repo) -> Self {
        self ^= other;
        self
    }
}

impl BitXorAssign<&Repo> for RepoSet {
    fn bitxor_assign(&mut self, other: &Repo) {
        self.repos = OrderedSet(self.repos.bitxor(&IndexSet::from([other.clone()])));
        self.repos.sort();
    }
}

impl Sub<&Repo> for RepoSet {
    type Output = Self;

    fn sub(mut self, other: &Repo) -> Self {
        self -= other;
        self
    }
}

impl SubAssign<&Repo> for RepoSet {
    fn sub_assign(&mut self, other: &Repo) {
        if self.repos.remove(other) {
            self.repos.sort();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::dep::Cpv;
    use crate::pkg::Package;
    use crate::repo::{fake, Contains, Repository};
    use crate::test::assert_ordered_eq;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn test_cmp_traits() {
        let s1 = RepoSet::new([]);
        let s2 = RepoSet::new([]);
        assert_eq!(s1, s2);
        assert_eq!(hash(s1), hash(s2));

        // different parameter order are still sorted lexically by repo id
        let r1: Repo = fake::Repo::new("r1", 0).into();
        let r2: Repo = fake::Repo::new("r2", 0).into();
        let s1 = RepoSet::new([&r1, &r2]);
        let s2 = RepoSet::new([&r2, &r1]);
        assert_eq!(s1, s2);
        assert_eq!(hash(s1), hash(s2));

        // higher priority repos come before lower priority ones
        let r1: Repo = fake::Repo::new("r1", -1).into();
        let r2: Repo = fake::Repo::new("r2", 0).into();
        let s1 = RepoSet::new([&r1]);
        let s2 = RepoSet::new([&r2]);
        assert!(s2 < s1);
        assert_ne!(s1, s2);
        assert_ne!(hash(s1), hash(s2));
    }

    #[test]
    fn test_repos() {
        let s = RepoSet::new([]);
        assert!(s.repos().is_empty());

        let r1: Repo = fake::Repo::new("r1", 0).into();
        let r2: Repo = fake::Repo::new("r2", 0).into();
        let s = RepoSet::new([&r1, &r2]);
        assert_ordered_eq(s.repos(), [&r1, &r2]);
        // different parameter order are still sorted lexically by repo id
        let s = RepoSet::new([&r2, &r1]);
        assert_ordered_eq(s.repos(), [&r1, &r2]);

        // higher priority repos come before lower priority ones
        let r1: Repo = fake::Repo::new("r1", -1).into();
        let r2: Repo = fake::Repo::new("r2", 0).into();
        let s = RepoSet::new([&r1, &r2]);
        assert_ordered_eq(s.repos(), [&r2, &r1]);
    }

    #[test]
    fn test_repo_traits() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let fake_repo = fake::Repo::new("fake", 0);

        let f_repo: Repo = fake_repo.into();
        let cpv = Cpv::new("cat/pkg-1").unwrap();

        // empty repo set
        let s = RepoSet::new([]);
        assert!(s.categories().is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
        assert!(s.iter().next().is_none());
        assert!(s.iter_restrict(&cpv).next().is_none());
        assert!(!s.contains(&cpv));

        // repo set with no pkgs
        let s = RepoSet::new([&t.repo, &f_repo]);
        assert!(s.categories().is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
        assert!(s.iter().next().is_none());
        assert!(s.iter_restrict(&cpv).next().is_none());
        assert!(!s.contains(&cpv));

        // single ebuild
        t.create_ebuild("cat/pkg-1", &[]).unwrap();
        assert_eq!(s.categories(), ["cat"]);
        assert_eq!(s.packages("cat"), ["pkg"]);
        assert_eq!(s.versions("cat", "pkg"), [Version::new("1").unwrap()]);
        assert_eq!(s.len(), 1);
        assert!(!s.is_empty());
        assert!(s.iter().next().is_some());
        assert!(s.iter_restrict(&cpv).next().is_some());
        assert!(s.contains(&cpv));

        // multiple pkgs of different types
        let fake_repo = fake::Repo::new("fake", 0).pkgs(["cat/pkg-1"]);
        let f_repo: Repo = fake_repo.into();
        let s = RepoSet::new([&t.repo, &f_repo]);
        assert_eq!(s.categories(), ["cat"]);
        assert_eq!(s.packages("cat"), ["pkg"]);
        assert_eq!(s.versions("cat", "pkg"), [Version::new("1").unwrap()]);
        assert_eq!(s.len(), 2);
        assert!(s.contains(&cpv));
        assert_eq!(s.iter().count(), 2);
        assert_eq!(s.iter_restrict(&cpv).count(), 2);
        let pkg = s.iter_restrict(&cpv).next().unwrap();
        // pkg restriction only matches the repo's pkg it came from
        assert_eq!(s.iter_restrict(&pkg).count(), 1);
        assert_eq!(s.iter_restrict(&pkg).next().unwrap().repo().id(), "fake");
    }

    #[test]
    fn test_bit_ops() {
        let cpv1 = Cpv::new("cat/pkg-1").unwrap();
        let cpv2 = Cpv::new("cat/pkg-2").unwrap();
        let cpv3 = Cpv::new("cat/pkg-3").unwrap();
        let cpv4 = Cpv::new("cat/pkg-4").unwrap();
        let r1: Repo = fake::Repo::new("1", 0).pkgs([&cpv1]).into();
        let r2: Repo = fake::Repo::new("2", 0).pkgs([&cpv2]).into();
        let r3: Repo = fake::Repo::new("3", 0).pkgs([&cpv3]).into();
        let r4: Repo = fake::Repo::new("4", 0).pkgs([&cpv4]).into();

        let mut s = RepoSet::new([]);
        assert!(!s.contains(&cpv1));

        // combine repo set and repo
        s |= &r1;
        assert!(s.contains(&cpv1));
        // combine repo set and repo set
        s |= &RepoSet::new([&r2]);
        assert!(s.contains(&cpv2));
        // combine repo set and repo
        let s = s | &r3;
        assert!(s.contains(&cpv3));
        // combine repo set and repo set
        let s = s | &RepoSet::new([&r4]);
        assert!(s.contains(&cpv4));

        // subtract repo set and repo set
        let s = s - &RepoSet::new([&r4]);
        assert!(!s.contains(&cpv4));
        // subtract repo set and repo
        let mut s = s - &r3;
        assert!(!s.contains(&cpv3));
        // subtract repo set and repo set
        s -= &RepoSet::new([&r2]);
        assert!(!s.contains(&cpv2));
        // subtract repo set and repo
        s -= &r1;
        assert!(!s.contains(&cpv1));
    }
}
