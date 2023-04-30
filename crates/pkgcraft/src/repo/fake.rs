use std::fmt;
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use tracing::warn;

use crate::config::RepoConfig;
use crate::dep::{Cpv, TryIntoCpv, Version};
use crate::pkg::fake::Pkg;
use crate::restrict::{Restrict, Restriction};
use crate::Error;

use super::{make_repo_traits, PkgRepository, RepoFormat, Repository};

type VersionMap = IndexMap<String, IndexSet<Version>>;
type PkgMap = IndexMap<String, VersionMap>;

#[derive(Debug, Default, Clone)]
pub struct Repo {
    id: String,
    repo_config: RepoConfig,
    pkgmap: PkgMap,
    cpvs: IndexSet<Cpv>,
}

make_repo_traits!(Repo);

impl Repo {
    pub fn new(id: &str, priority: i32) -> Self {
        let repo_config = RepoConfig { priority, ..Default::default() };
        Self {
            id: id.to_string(),
            repo_config,
            ..Default::default()
        }
    }

    pub fn pkgs<I, C>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: TryIntoCpv,
    {
        self.extend(iter);
        self
    }

    pub fn from_path<P: AsRef<Utf8Path>>(id: &str, priority: i32, path: P) -> crate::Result<Self> {
        let path = path.as_ref();
        let data = fs::read_to_string(path).map_err(|e| Error::RepoInit(e.to_string()))?;
        let repo_config = RepoConfig {
            location: Utf8PathBuf::from(path),
            priority,
            ..Default::default()
        };
        let mut repo = Self {
            id: id.to_string(),
            repo_config,
            ..Default::default()
        };
        repo.extend(data.lines());
        Ok(repo)
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.repo_config
    }
}

impl<C: TryIntoCpv> Extend<C> for Repo {
    fn extend<T: IntoIterator<Item = C>>(&mut self, iter: T) {
        let orig_len = self.cpvs.len();
        for s in iter {
            match s.try_into_cpv() {
                Ok(cpv) => {
                    self.cpvs.insert(cpv);
                }
                Err(e) => warn!("{e}"),
            }
        }

        if orig_len != self.cpvs.len() {
            self.cpvs.sort();

            // recreate entire PkgMap structure to preserve correct ordering
            let mut pkgmap = PkgMap::new();
            for cpv in &self.cpvs {
                pkgmap
                    .entry(cpv.category().into())
                    .or_insert_with(VersionMap::new)
                    .entry(cpv.package().into())
                    .or_insert_with(IndexSet::new)
                    .insert(cpv.version().clone());
            }
            self.pkgmap = pkgmap;
        }
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: fake repo", self.id)
    }
}

impl PkgRepository for Repo {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type Iter<'a> = Iter<'a> where Self: 'a;
    type IterRestrict<'a> = IterRestrict<'a> where Self: 'a;

    // TODO: cache categories/packages/versions values in OnceCell fields?
    fn categories(&self) -> Vec<String> {
        self.pkgmap.keys().map(|k| k.to_string()).collect()
    }

    fn packages(&self, cat: &str) -> Vec<String> {
        self.pkgmap
            .get(cat)
            .map(|pkgs| pkgs.keys().map(|k| k.to_string()).collect())
            .unwrap_or_default()
    }

    fn versions(&self, cat: &str, pkg: &str) -> Vec<Version> {
        self.pkgmap
            .get(cat)
            .and_then(|pkgs| pkgs.get(pkg))
            .map(|vers| vers.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn len(&self) -> usize {
        self.cpvs.len()
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
        self.repo_config.format
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn priority(&self) -> i32 {
        self.repo_config.priority
    }

    fn path(&self) -> &Utf8Path {
        &self.repo_config.location
    }

    fn sync(&self) -> crate::Result<()> {
        self.repo_config.sync()
    }
}

impl<'a> IntoIterator for &'a Repo {
    type Item = Pkg<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: self.cpvs.iter(),
            repo: self,
        }
    }
}

#[derive(Debug)]
pub struct Iter<'a> {
    iter: indexmap::set::Iter<'a, Cpv>,
    repo: &'a Repo,
}

impl<'a> Iterator for Iter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|cpv| Pkg::new(cpv, self.repo))
    }
}

#[derive(Debug)]
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
    use std::str::FromStr;

    use crate::dep::Dep;
    use crate::pkg::Package;
    use crate::repo::Contains;

    use super::*;

    #[test]
    fn test_id() {
        let repo = Repo::new("fake", 0);
        assert_eq!(repo.id(), "fake");
    }

    #[test]
    fn test_categories() {
        let mut repo = Repo::new("fake", 0);
        // empty repo
        assert!(repo.categories().is_empty());
        // existing pkgs
        repo.extend(["cat1/pkg-a-1", "cat1/pkg-a-2", "cat2/pkg-b-3"]);
        assert_eq!(repo.categories(), ["cat1", "cat2"])
    }

    #[test]
    fn test_packages() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", 0);
        assert!(repo.packages("cat").is_empty());
        // existing pkgs
        repo.extend(["cat1/pkg-a-1", "cat1/pkg-a-2", "cat2/pkg-b-3"]);
        assert!(repo.packages("cat").is_empty());
        assert_eq!(repo.packages("cat1"), ["pkg-a"]);
        assert_eq!(repo.packages("cat2"), ["pkg-b"]);
    }

    #[test]
    fn test_versions() {
        let ver = |s: &str| Version::new(s).unwrap();
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", 0);
        assert!(repo.versions("cat", "pkg").is_empty());
        // existing pkgs
        repo.extend(["cat1/pkg-a-1", "cat1/pkg-a-2", "cat2/pkg-b-3"]);
        assert!(repo.versions("cat", "pkg").is_empty());
        assert_eq!(repo.versions("cat1", "pkg-a"), [ver("1"), ver("2")]);
        assert_eq!(repo.versions("cat2", "pkg-b"), [ver("3")]);
    }

    #[test]
    fn test_len() {
        let mut repo = Repo::new("fake", 0);
        assert_eq!(repo.len(), 0);
        repo.extend(["cat/pkg-0"]);
        assert_eq!(repo.len(), 1);
        repo.extend(["cat/pkg-0", "cat1/pkg1-1", "cat2/pkg2-2"]);
        assert_eq!(repo.len(), 3);
    }

    #[test]
    fn test_extend() {
        let mut repo = Repo::new("fake", 0).pkgs(["cat/pkg-2"]);
        let cpvs: Vec<_> = repo.iter().map(|pkg| pkg.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-2"]);

        // add single cpv
        repo.extend(["cat/pkg-0"]);
        let cpvs: Vec<_> = repo.iter().map(|pkg| pkg.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-0", "cat/pkg-2"]);

        // add multiple cpvs
        repo.extend(["cat/pkg-3", "cat/pkg-1", "a/b-0"]);
        let cpvs: Vec<_> = repo.iter().map(|pkg| pkg.cpv().to_string()).collect();
        assert_eq!(cpvs, ["a/b-0", "cat/pkg-0", "cat/pkg-1", "cat/pkg-2", "cat/pkg-3"]);
    }

    #[test]
    fn test_contains() {
        let repo = Repo::new("fake", 0).pkgs(["cat/pkg-0"]);

        // path is always false due to fake repo
        assert!(!repo.contains("cat/pkg"));

        // cpv
        let cpv = Cpv::new("cat/pkg-0").unwrap();
        assert!(repo.contains(&cpv));
        let cpv = Cpv::new("cat/pkg-1").unwrap();
        assert!(!repo.contains(&cpv));

        // unversioned dep
        let a = Dep::from_str("cat/pkg").unwrap();
        assert!(repo.contains(&a));
        let a = Dep::from_str("cat/pkg-a").unwrap();
        assert!(!repo.contains(&a));
    }

    #[test]
    fn test_iter() {
        let repo = Repo::new("fake", 0).pkgs(["cat/pkg-0", "acat/bpkg-1"]);
        let cpvs: Vec<_> = repo.iter().map(|pkg| pkg.cpv().to_string()).collect();
        assert_eq!(cpvs, ["acat/bpkg-1", "cat/pkg-0"]);
    }
}
