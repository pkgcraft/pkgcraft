use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use camino::Utf8Path;
use indexmap::IndexSet;

use crate::config::{RepoConfig, Settings};
use crate::dep::{Cpn, Cpv, Dep, Version};
use crate::pkg::ebuild::configured::Pkg;
use crate::repo::{make_repo_traits, PkgRepository, RepoFormat, Repository};
use crate::restrict::{Restrict, Restriction};
use crate::traits::Contains;

use super::EbuildRepo;

/// Configured ebuild repository.
#[derive(Debug)]
struct Repo {
    raw: EbuildRepo,
    settings: Arc<Settings>,
}

impl<'a> From<&'a ConfiguredRepo> for &'a EbuildRepo {
    fn from(repo: &'a ConfiguredRepo) -> Self {
        &repo.0.raw
    }
}

#[derive(Debug, Clone)]
pub struct ConfiguredRepo(Arc<Repo>);

impl Deref for ConfiguredRepo {
    type Target = EbuildRepo;

    fn deref(&self) -> &Self::Target {
        &self.0.raw
    }
}

impl PartialEq for ConfiguredRepo {
    fn eq(&self, other: &Self) -> bool {
        self.path() == other.path()
    }
}

impl Eq for ConfiguredRepo {}

impl Hash for ConfiguredRepo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path().hash(state);
    }
}

make_repo_traits!(ConfiguredRepo);

impl ConfiguredRepo {
    pub(super) fn new(raw: EbuildRepo, settings: Arc<Settings>) -> Self {
        ConfiguredRepo(Arc::new(Repo { raw, settings }))
    }

    pub(crate) fn repo_config(&self) -> &RepoConfig {
        self.0.raw.repo_config()
    }
}

impl fmt::Display for ConfiguredRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.raw)
    }
}

impl PkgRepository for ConfiguredRepo {
    type Pkg = Pkg;
    type IterCpv = <EbuildRepo as PkgRepository>::IterCpv;
    type Iter = Iter;
    type IterRestrict = IterRestrict;

    fn categories(&self) -> IndexSet<String> {
        self.0.raw.categories()
    }

    fn packages(&self, cat: &str) -> IndexSet<String> {
        self.0.raw.packages(cat)
    }

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        self.0.raw.versions(cat, pkg)
    }

    fn len(&self) -> usize {
        self.0.raw.len()
    }

    fn iter_cpv(&self) -> Self::IterCpv {
        self.0.raw.iter_cpv()
    }

    fn iter(&self) -> Self::Iter {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict {
        IterRestrict {
            iter: self.into_iter(),
            restrict: val.into(),
        }
    }
}

impl Contains<&Cpn> for ConfiguredRepo {
    fn contains(&self, cpn: &Cpn) -> bool {
        self.0.raw.contains(cpn)
    }
}

impl Contains<&Cpv> for ConfiguredRepo {
    fn contains(&self, cpv: &Cpv) -> bool {
        self.0.raw.contains(cpv)
    }
}

impl Contains<&Dep> for ConfiguredRepo {
    fn contains(&self, dep: &Dep) -> bool {
        self.0.raw.contains(dep)
    }
}

impl Repository for ConfiguredRepo {
    fn format(&self) -> RepoFormat {
        RepoFormat::Configured
    }

    fn id(&self) -> &str {
        self.0.raw.id()
    }

    fn priority(&self) -> i32 {
        self.0.raw.priority()
    }

    fn path(&self) -> &Utf8Path {
        self.0.raw.path()
    }

    fn sync(&self) -> crate::Result<()> {
        self.0.raw.sync()
    }
}

impl IntoIterator for &ConfiguredRepo {
    type Item = Pkg;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: super::Iter::new(&self.0.raw, None),
            repo: self.clone(),
        }
    }
}

pub struct Iter {
    iter: super::Iter,
    repo: ConfiguredRepo,
}

impl Iterator for Iter {
    type Item = Pkg;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|pkg| Pkg::new(self.repo.clone(), self.repo.0.settings.clone(), pkg))
    }
}

pub struct IterRestrict {
    iter: Iter,
    restrict: Restrict,
}

impl Iterator for IterRestrict {
    type Item = Pkg;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|pkg| self.restrict.matches(pkg))
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::dep::Cpv;
    use crate::pkg::Package;
    use crate::repo::PkgRepository;
    use crate::restrict::dep::Restrict as DepRestrict;

    #[test]
    fn iter() {
        let mut config = Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();
        let configured = repo.configure(&config);
        repo.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        repo.create_raw_pkg("cat1/pkg-1", &[]).unwrap();
        let mut iter = configured.iter();
        for cpv in ["cat1/pkg-1", "cat2/pkg-1"] {
            let pkg = iter.next();
            assert_eq!(pkg.map(|p| format!("{}", p.cpv())), Some(cpv.to_string()));
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn iter_restrict() {
        let mut config = Config::default();
        let repo = config.temp_repo("test", 0, None).unwrap();
        let configured = repo.configure(&config);
        repo.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        repo.create_raw_pkg("cat/pkg-2", &[]).unwrap();

        // single match via CPV
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        let iter = configured.iter_restrict(&cpv);
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
