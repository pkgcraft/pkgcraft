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
#[derive(Debug, Clone)]
pub struct ConfiguredRepo {
    raw: EbuildRepo,
    settings: Arc<Settings>,
}

impl<'a> From<&'a ConfiguredRepo> for &'a EbuildRepo {
    fn from(repo: &'a ConfiguredRepo) -> Self {
        &repo.raw
    }
}

impl Deref for ConfiguredRepo {
    type Target = EbuildRepo;

    fn deref(&self) -> &Self::Target {
        &self.raw
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
        ConfiguredRepo { raw, settings }
    }

    pub(crate) fn repo_config(&self) -> &RepoConfig {
        self.raw.repo_config()
    }
}

impl fmt::Display for ConfiguredRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl PkgRepository for ConfiguredRepo {
    type Pkg = Pkg;
    type IterCpv = <EbuildRepo as PkgRepository>::IterCpv;
    type IterCpvRestrict = <EbuildRepo as PkgRepository>::IterCpvRestrict;
    type Iter = Iter;
    type IterRestrict = IterRestrict;

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

    fn iter_cpv(&self) -> Self::IterCpv {
        self.raw.iter_cpv()
    }

    fn iter_cpv_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpvRestrict {
        self.raw.iter_cpv_restrict(value)
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
        self.raw.contains(cpn)
    }
}

impl Contains<&Cpv> for ConfiguredRepo {
    fn contains(&self, cpv: &Cpv) -> bool {
        self.raw.contains(cpv)
    }
}

impl Contains<&Dep> for ConfiguredRepo {
    fn contains(&self, dep: &Dep) -> bool {
        self.raw.contains(dep)
    }
}

impl Repository for ConfiguredRepo {
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

impl IntoIterator for &ConfiguredRepo {
    type Item = Pkg;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: super::Iter::new(&self.raw, None),
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
            .map(|pkg| Pkg::new(self.repo.clone(), self.repo.settings.clone(), pkg))
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
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        temp.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkg-1", &[]).unwrap();
        let configured = temp.configure(&config);
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
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        temp.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        temp.create_raw_pkg("cat/pkg-2", &[]).unwrap();
        let configured = temp.configure(&config);

        // single match via CPV
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        let iter = configured.iter_restrict(&cpv);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, [cpv.to_string()]);

        // single match via package
        let pkg = temp.iter().next().unwrap();
        let iter = temp.iter_restrict(&pkg);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, [pkg.cpv().to_string()]);

        // multiple matches
        let restrict = DepRestrict::package("pkg");
        let iter = temp.iter_restrict(restrict);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-1", "cat/pkg-2"]);
    }
}
