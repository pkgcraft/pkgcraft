use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Sub, SubAssign};

use indexmap::IndexSet;

use crate::atom;
use crate::pkg::Pkg;
use crate::repo::{Repo, Repository};
use crate::restrict::Restrict;

use super::make_contains_atom;

/// Ordered set of repos
#[derive(Debug, Clone)]
pub struct RepoSet(IndexSet<Repo>);

impl PartialEq for RepoSet {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for RepoSet {}

impl Hash for RepoSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for r in self.repos() {
            r.hash(state);
        }
    }
}

impl Ord for RepoSet {
    fn cmp(&self, other: &Self) -> Ordering {
        self.repos().iter().cmp(other.repos().iter())
    }
}

impl PartialOrd for RepoSet {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl RepoSet {
    pub fn new<'a, I>(repos: I) -> Self
    where
        I: IntoIterator<Item = &'a Repo>,
    {
        let mut repos: IndexSet<_> = repos.into_iter().cloned().collect();
        repos.sort();
        Self(repos)
    }

    pub fn repos(&self) -> &IndexSet<Repo> {
        &self.0
    }

    pub fn iter(&self) -> PkgIter {
        self.into_iter()
    }

    pub fn iter_restrict<T: Into<Restrict>>(&self, val: T) -> PkgIter {
        let restrict = val.into();
        PkgIter(Box::new(
            self.0
                .iter()
                .flat_map(move |r| r.iter_restrict(restrict.clone())),
        ))
    }
}

make_contains_atom!(RepoSet, [atom::Atom, &atom::Atom]);

// TODO: Use type alias impl trait support for IntoIterator implementation when stable in order to
// replace boxed type with a generic type.
//
// See https://github.com/rust-lang/rust/issues/63063
pub struct PkgIter<'a>(Box<dyn Iterator<Item = Pkg<'a>> + 'a>);

impl<'a> IntoIterator for &'a RepoSet {
    type Item = Pkg<'a>;
    type IntoIter = PkgIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PkgIter(Box::new(self.0.iter().flat_map(|r| r.into_iter())))
    }
}

impl<'a> Iterator for PkgIter<'a> {
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
        self.0 = &self.0 & &other.0;
        self.0.sort();
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
        self.0 = &self.0 | &other.0;
        self.0.sort();
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
        self.0 = &self.0 ^ &other.0;
        self.0.sort();
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
        self.0 = &self.0 - &other.0;
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
        self.0 = &self.0 & &IndexSet::from([other.clone()]);
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
        if self.0.insert(other.clone()) {
            self.0.sort();
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
        self.0 = &self.0 ^ &IndexSet::from([other.clone()]);
        self.0.sort();
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
        if self.0.remove(other) {
            self.0.sort();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::pkg::Package;
    use crate::repo::{fake, Contains};
    use crate::test::eq_ordered;
    use crate::utils::hash;

    use super::*;

    #[test]
    fn test_cmp_traits() {
        let s1 = RepoSet::new([]);
        let s2 = RepoSet::new([]);
        assert_eq!(s1, s2);
        assert_eq!(hash(s1), hash(s2));

        // different parameter order are still sorted lexically by repo id
        let r1: Repo = fake::Repo::new("r1", 0, []).unwrap().into();
        let r2: Repo = fake::Repo::new("r2", 0, []).unwrap().into();
        let s1 = RepoSet::new([&r1, &r2]);
        let s2 = RepoSet::new([&r2, &r1]);
        assert_eq!(s1, s2);
        assert_eq!(hash(s1), hash(s2));

        // higher priority repos come before lower priority ones
        let r1: Repo = fake::Repo::new("r1", -1, []).unwrap().into();
        let r2: Repo = fake::Repo::new("r2", 0, []).unwrap().into();
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

        let r1: Repo = fake::Repo::new("r1", 0, []).unwrap().into();
        let r2: Repo = fake::Repo::new("r2", 0, []).unwrap().into();
        let s = RepoSet::new([&r1, &r2]);
        assert!(eq_ordered(s.repos(), [&r1, &r2]));
        // different parameter order are still sorted lexically by repo id
        let s = RepoSet::new([&r2, &r1]);
        assert!(eq_ordered(s.repos(), [&r1, &r2]));

        // higher priority repos come before lower priority ones
        let r1: Repo = fake::Repo::new("r1", -1, []).unwrap().into();
        let r2: Repo = fake::Repo::new("r2", 0, []).unwrap().into();
        let s = RepoSet::new([&r1, &r2]);
        assert!(eq_ordered(s.repos(), [&r2, &r1]));
    }

    #[test]
    fn test_iter_and_contains() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, ebuild_repo) = config.temp_repo("test", 0).unwrap();
        let fake_repo = fake::Repo::new("fake", 0, []).unwrap();

        let e_repo: Repo = ebuild_repo.into();
        let f_repo: Repo = fake_repo.into();
        let cpv = atom::cpv("cat/pkg-1").unwrap();

        // empty repo set
        let s = RepoSet::new([]);
        assert!(s.iter().next().is_none());
        assert!(s.iter_restrict(&cpv).next().is_none());
        assert!(!s.contains(&cpv));
        assert!(!s.contains(cpv.clone()));

        // repo set with no pkgs
        let s = RepoSet::new([&e_repo, &f_repo]);
        assert!(s.iter().next().is_none());
        assert!(s.iter_restrict(&cpv).next().is_none());
        assert!(!s.contains(&cpv));
        assert!(!s.contains(cpv.clone()));

        // single ebuild
        t.create_ebuild("cat/pkg-1", []).unwrap();
        assert!(s.iter().next().is_some());
        assert!(s.iter_restrict(&cpv).next().is_some());
        assert!(s.contains(&cpv));
        assert!(s.contains(cpv.clone()));

        // multiple pkgs of different types
        let fake_repo = fake::Repo::new("fake", 0, ["cat/pkg-1"]).unwrap();
        let f_repo: Repo = fake_repo.into();
        let s = RepoSet::new([&e_repo, &f_repo]);
        assert!(s.contains(&cpv));
        assert!(s.contains(cpv.clone()));
        assert_eq!(s.iter().count(), 2);
        assert_eq!(s.iter_restrict(&cpv).count(), 2);
        let pkg = s.iter_restrict(&cpv).next().unwrap();
        // pkg restriction only matches the repo's pkg it came from
        assert_eq!(s.iter_restrict(&pkg).count(), 1);
        assert_eq!(s.iter_restrict(&pkg).next().unwrap().repo().id(), "fake");
    }

    #[test]
    fn test_bit_ops() {
        let r1: Repo = fake::Repo::new("1", 0, ["cat/pkg-1"]).unwrap().into();
        let r2: Repo = fake::Repo::new("2", 0, ["cat/pkg-2"]).unwrap().into();
        let r3: Repo = fake::Repo::new("3", 0, ["cat/pkg-3"]).unwrap().into();
        let r4: Repo = fake::Repo::new("3", 0, ["cat/pkg-3"]).unwrap().into();
        let cpv1 = atom::cpv("cat/pkg-1").unwrap();
        let cpv2 = atom::cpv("cat/pkg-2").unwrap();
        let cpv3 = atom::cpv("cat/pkg-3").unwrap();
        let cpv4 = atom::cpv("cat/pkg-3").unwrap();

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
