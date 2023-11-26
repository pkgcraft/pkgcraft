use std::fmt;
use std::sync::Arc;

use camino::Utf8Path;
use indexmap::IndexSet;

use crate::config::{RepoConfig, Settings};
use crate::dep::Version;
use crate::pkg::ebuild::configured::Pkg;
use crate::repo::{make_repo_traits, PkgRepository, RepoFormat, Repository};
use crate::restrict::{Restrict, Restriction};

/// Configured ebuild repository.
#[derive(Debug)]
pub struct Repo {
    raw: Arc<super::Repo>,
    settings: Arc<Settings>,
}

impl<'a> From<&'a Repo> for &'a super::Repo {
    fn from(repo: &'a Repo) -> Self {
        repo.raw.as_ref()
    }
}

make_repo_traits!(Repo);

impl Repo {
    pub(super) fn new(raw: Arc<super::Repo>, settings: Arc<Settings>) -> Self {
        Repo { raw, settings }
    }

    pub(crate) fn repo_config(&self) -> &RepoConfig {
        self.raw.repo_config()
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl PkgRepository for Repo {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type IterCpv<'a> = <super::Repo as PkgRepository>::IterCpv<'a> where Self: 'a;
    type Iter<'a> = Iter<'a> where Self: 'a;
    type IterRestrict<'a> = IterRestrict<'a> where Self: 'a;

    fn categories(&self) -> IndexSet<String> {
        self.raw.categories()
    }

    fn packages(&self, cat: &str) -> IndexSet<String> {
        self.raw.packages(cat)
    }

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        self.raw.versions(cat, pkg)
    }

    fn len(&self) -> usize {
        self.raw.len()
    }

    fn iter_cpv(&self) -> Self::IterCpv<'_> {
        self.raw.iter_cpv()
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict<'_> {
        IterRestrict {
            iter: self.into_iter(),
            restrict: val.into(),
        }
    }
}

impl Repository for Repo {
    fn format(&self) -> RepoFormat {
        RepoFormat::Configured
    }

    fn id(&self) -> &str {
        self.raw.id()
    }

    fn priority(&self) -> i32 {
        self.raw.priority()
    }

    fn path(&self) -> &Utf8Path {
        self.raw.path()
    }

    fn sync(&self) -> crate::Result<()> {
        self.raw.sync()
    }
}

impl<'a> IntoIterator for &'a Repo {
    type Item = Pkg<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: super::Iter::new(self.raw.as_ref(), None),
            repo: self,
        }
    }
}

pub struct Iter<'a> {
    iter: super::Iter<'a>,
    repo: &'a Repo,
}

impl<'a> Iterator for Iter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|pkg| Pkg::new(self.repo, self.repo.settings.as_ref(), pkg))
    }
}

pub struct IterRestrict<'a> {
    iter: Iter<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for IterRestrict<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|pkg| self.restrict.matches(pkg))
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::dep::Cpv;
    use crate::pkg::Package;
    use crate::restrict::dep::Restrict as DepRestrict;

    use super::*;

    #[test]
    fn test_iter() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo().configure(&config);
        t.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        t.create_raw_pkg("cat1/pkg-1", &[]).unwrap();
        let mut iter = repo.iter();
        for cpv in ["cat1/pkg-1", "cat2/pkg-1"] {
            let pkg = iter.next();
            assert_eq!(pkg.map(|p| format!("{}", p.cpv())), Some(cpv.to_string()));
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_iter_restrict() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo().configure(&config);
        t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        t.create_raw_pkg("cat/pkg-2", &[]).unwrap();

        // single match via CPV
        let cpv = Cpv::new("cat/pkg-1").unwrap();
        let iter = repo.iter_restrict(&cpv);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, [cpv.to_string()]);

        // single match via package
        let pkg = repo.iter().next().unwrap();
        let iter = repo.iter_restrict(&pkg);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, [pkg.cpv().to_string()]);

        // multiple matches
        let restrict = DepRestrict::package("pkg");
        let iter = repo.iter_restrict(restrict);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-1", "cat/pkg-2"]);
    }
}
