use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::iter::Flatten;
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

    pub fn iter_restrict<T: Into<Restrict>>(&self, val: T) -> RestrictPkgIter {
        let restrict = val.into();
        #[allow(clippy::needless_collect)]
        let pkgs: Vec<_> = self
            .0
            .iter()
            .map(|r| r.iter_restrict(restrict.clone()))
            .collect();
        RestrictPkgIter(pkgs.into_iter().flatten())
    }
}

make_contains_atom!(RepoSet, [atom::Atom, &atom::Atom]);

pub struct PkgIter<'a>(Flatten<std::vec::IntoIter<super::PkgIter<'a>>>);

impl<'a> IntoIterator for &'a RepoSet {
    type Item = Pkg<'a>;
    type IntoIter = PkgIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        #[allow(clippy::needless_collect)]
        let pkgs: Vec<_> = self.0.iter().map(|r| r.into_iter()).collect();
        PkgIter(pkgs.into_iter().flatten())
    }
}

impl<'a> Iterator for PkgIter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

pub struct RestrictPkgIter<'a>(Flatten<std::vec::IntoIter<super::RestrictPkgIter<'a>>>);

impl<'a> Iterator for RestrictPkgIter<'a> {
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

    use super::*;

    #[test]
    fn test_repo_set_iter_and_contains() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, ebuild_repo) = config.temp_repo("test", 0).unwrap();
        let fake_repo = fake::Repo::new("fake", 0, []).unwrap();

        let e_repo: Repo = ebuild_repo.into();
        let f_repo: Repo = fake_repo.into();
        let cpv = atom::cpv("cat/pkg-1").unwrap();

        // empty repo set
        let empty_set = RepoSet::new([]);
        assert!(empty_set.iter().next().is_none());
        assert!(empty_set.iter_restrict(&cpv).next().is_none());
        assert!(!empty_set.contains(&cpv));
        assert!(!empty_set.contains(cpv.clone()));

        // repo set with no pkgs
        let repo = RepoSet::new([&e_repo, &f_repo]);
        assert!(repo.iter().next().is_none());
        assert!(repo.iter_restrict(&cpv).next().is_none());
        assert!(!repo.contains(&cpv));
        assert!(!repo.contains(cpv.clone()));

        // single ebuild
        t.create_ebuild("cat/pkg-1", []).unwrap();
        assert!(repo.iter().next().is_some());
        assert!(repo.iter_restrict(&cpv).next().is_some());
        assert!(repo.contains(&cpv));
        assert!(repo.contains(cpv.clone()));

        // multiple pkgs of different types
        let fake_repo = fake::Repo::new("fake", 0, ["cat/pkg-1"]).unwrap();
        let f_repo: Repo = fake_repo.into();
        let repo = RepoSet::new([&e_repo, &f_repo]);
        assert!(repo.contains(&cpv));
        assert!(repo.contains(cpv.clone()));
        assert_eq!(repo.iter().count(), 2);
        assert_eq!(repo.iter_restrict(&cpv).count(), 2);
        let pkg = repo.iter_restrict(&cpv).next().unwrap();
        // pkg restriction only matches the repo's pkg it came from
        assert_eq!(repo.iter_restrict(&pkg).count(), 1);
        assert_eq!(repo.iter_restrict(&pkg).next().unwrap().repo().id(), "fake");
    }

    #[test]
    fn test_repo_set_ops() {
        let repo1: Repo = fake::Repo::new("1", 0, ["cat/pkg-1"]).unwrap().into();
        let repo2: Repo = fake::Repo::new("2", 0, ["cat/pkg-2"]).unwrap().into();
        let repo3: Repo = fake::Repo::new("3", 0, ["cat/pkg-3"]).unwrap().into();
        let repo4: Repo = fake::Repo::new("3", 0, ["cat/pkg-3"]).unwrap().into();
        let cpv1 = atom::cpv("cat/pkg-1").unwrap();
        let cpv2 = atom::cpv("cat/pkg-2").unwrap();
        let cpv3 = atom::cpv("cat/pkg-3").unwrap();
        let cpv4 = atom::cpv("cat/pkg-3").unwrap();

        let mut repo = RepoSet::new([]);
        assert!(!repo.contains(&cpv1));

        // combine repo set and repo
        repo |= &repo1;
        assert!(repo.contains(&cpv1));
        // combine repo set and repo set
        repo |= &RepoSet::new([&repo2]);
        assert!(repo.contains(&cpv2));
        // combine repo set and repo
        let repo = repo | &repo3;
        assert!(repo.contains(&cpv3));
        // combine repo set and repo set
        let repo = repo | &RepoSet::new([&repo4]);
        assert!(repo.contains(&cpv4));

        // subtract repo set and repo set
        let repo = repo - &RepoSet::new([&repo4]);
        assert!(!repo.contains(&cpv4));
        // subtract repo set and repo
        let mut repo = repo - &repo3;
        assert!(!repo.contains(&cpv3));
        // subtract repo set and repo set
        repo -= &RepoSet::new([&repo2]);
        assert!(!repo.contains(&cpv2));
        // subtract repo set and repo
        repo -= &repo1;
        assert!(!repo.contains(&cpv1));
    }
}
