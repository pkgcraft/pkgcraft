use std::fmt;
use std::sync::Arc;

use camino::Utf8Path;
use indexmap::IndexSet;

use crate::config::{Config, RepoConfig};
use crate::dep::Version;
use crate::pkg::ebuild::configured::Pkg;
use crate::repo::{make_repo_traits, PkgRepository, RepoFormat, Repository};
use crate::restrict::{Restrict, Restriction};

/// Configured ebuild repository.
#[derive(Debug)]
pub struct Repo {
    raw: Arc<super::Repo>,
}

make_repo_traits!(Repo);

impl Repo {
    pub fn new(raw: &Arc<super::Repo>, _config: &Config) -> Self {
        Repo { raw: raw.clone() }
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
        self.raw.format()
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
        self.iter.next().map(|pkg| Pkg::new(self.repo, pkg))
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
