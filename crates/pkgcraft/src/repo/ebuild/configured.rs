use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use camino::Utf8Path;
use indexmap::IndexSet;

use crate::config::{RepoConfig, Settings};
use crate::dep::{Cpn, Cpv, Dep, Version};
use crate::pkg::ebuild::EbuildConfiguredPkg;
use crate::repo::{PkgRepository, Repository, make_repo_traits};
use crate::restrict::{Restrict, Restriction};
use crate::traits::Contains;

use super::EbuildRepo;

/// Configured ebuild repository.
#[derive(Clone)]
pub struct ConfiguredRepo {
    raw: EbuildRepo,
    settings: Arc<Settings>,
}

impl fmt::Debug for ConfiguredRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ConfiguredRepo")
            .field("id", &self.id())
            .finish()
    }
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
}

impl fmt::Display for ConfiguredRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl PkgRepository for ConfiguredRepo {
    type Pkg = EbuildConfiguredPkg;
    type IterCpn = <EbuildRepo as PkgRepository>::IterCpn;
    type IterCpnRestrict = <EbuildRepo as PkgRepository>::IterCpnRestrict;
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

    fn iter_cpn(&self) -> Self::IterCpn {
        self.raw.iter_cpn()
    }

    fn iter_cpn_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpnRestrict {
        self.raw.iter_cpn_restrict(value)
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
    fn config(&self) -> &RepoConfig {
        self.raw.config()
    }

    fn id(&self) -> &str {
        self.raw.id()
    }
}

impl IntoIterator for &ConfiguredRepo {
    type Item = crate::Result<EbuildConfiguredPkg>;
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
    type Item = crate::Result<EbuildConfiguredPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|x| {
            x.map(|pkg| {
                EbuildConfiguredPkg::new(self.repo.clone(), self.repo.settings.clone(), pkg)
            })
        })
    }
}

pub struct IterRestrict {
    iter: Iter,
    restrict: Restrict,
}

impl Iterator for IterRestrict {
    type Item = crate::Result<EbuildConfiguredPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|r| match r {
            Ok(pkg) if self.restrict.matches(&pkg) => Some(Ok(pkg)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::config::Config;
    use crate::dep::Cpv;
    use crate::pkg::Package;
    use crate::repo::PkgRepository;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::restrict::dep::Restrict as DepRestrict;
    use crate::test::assert_ordered_eq;

    #[test]
    fn iter() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat2/pkg-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkg-1", &[]).unwrap();
        let repo = repo.configure(&config);
        let pkgs: Vec<_> = repo.iter().try_collect().unwrap();
        assert_ordered_eq!(
            pkgs.iter().map(|p| p.cpv().to_string()),
            ["cat1/pkg-1", "cat2/pkg-1"]
        );
    }

    #[test]
    fn iter_restrict() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        temp.create_ebuild("cat/pkg-2", &[]).unwrap();
        let repo = repo.configure(&config);

        // single match via CPV
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        let pkgs: Vec<_> = repo.iter_restrict(&cpv).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), [cpv.to_string()]);

        // single match via package
        let pkg = repo.iter().next().unwrap().unwrap();
        let pkgs: Vec<_> = repo.iter_restrict(&pkg).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), [pkg.cpv().to_string()]);

        // multiple matches
        let restrict = DepRestrict::package("pkg");
        let pkgs: Vec<_> = repo.iter_restrict(restrict).try_collect().unwrap();
        assert_ordered_eq!(
            pkgs.iter().map(|p| p.cpv().to_string()),
            ["cat/pkg-1", "cat/pkg-2"]
        );
    }
}
