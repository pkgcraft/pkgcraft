use std::fmt;
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use tracing::warn;

use crate::atom::{self, Atom};
use crate::config::RepoConfig;
use crate::pkg::fake::Pkg;
use crate::restrict::{Restrict, Restriction};
use crate::Error;

use super::{make_repo_traits, Contains, PkgRepository, Repository};

type VersionMap = IndexMap<String, IndexSet<String>>;
type PkgMap = IndexMap<String, VersionMap>;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct PkgCache {
    pkgmap: PkgMap,
    cpvs: IndexSet<Atom>,
}

impl PkgCache {
    fn new<'a, I: IntoIterator<Item = &'a str>>(cpvs: I) -> Self {
        let mut pkgs = Self::default();
        pkgs.extend(cpvs);
        pkgs
    }

    fn categories(&self) -> Vec<String> {
        self.pkgmap.clone().into_keys().collect()
    }

    fn packages<S: AsRef<str>>(&self, cat: S) -> Vec<String> {
        match self.pkgmap.get(cat.as_ref()) {
            Some(pkgs) => pkgs.clone().into_keys().collect(),
            None => vec![],
        }
    }

    fn versions<S: AsRef<str>>(&self, cat: S, pkg: S) -> Vec<String> {
        match self.pkgmap.get(cat.as_ref()) {
            Some(pkgs) => match pkgs.get(pkg.as_ref()) {
                Some(vers) => vers.clone().into_iter().collect(),
                None => vec![],
            },
            None => vec![],
        }
    }

    fn len(&self) -> usize {
        self.cpvs.len()
    }
}

impl<'a> IntoIterator for &'a PkgCache {
    type Item = &'a Atom;
    type IntoIter = PkgCacheIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PkgCacheIter {
            iter: self.cpvs.iter(),
        }
    }
}

#[derive(Debug)]
pub struct PkgCacheIter<'a> {
    iter: indexmap::set::Iter<'a, Atom>,
}

impl<'a> Iterator for PkgCacheIter<'a> {
    type Item = &'a Atom;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a> Extend<&'a str> for PkgCache {
    fn extend<T: IntoIterator<Item = &'a str>>(&mut self, iter: T) {
        // TODO: Currently the entire PkgMap structure is recreated in order to avoid having to
        // re-sort its nested nature.
        let mut pkgmap = PkgMap::new();

        for s in iter {
            match atom::cpv(s) {
                Ok(cpv) => {
                    self.cpvs.insert(cpv);
                }
                Err(e) => warn!("{e}"),
            }
        }

        self.cpvs.sort();

        for cpv in &self.cpvs {
            pkgmap
                .entry(cpv.category().into())
                .or_insert_with(VersionMap::new)
                .entry(cpv.package().into())
                .or_insert_with(IndexSet::new)
                .insert(cpv.version().unwrap().into());
        }

        self.pkgmap = pkgmap;
    }
}

#[derive(Debug, Default, Clone)]
pub struct Repo {
    id: String,
    repo_config: RepoConfig,
    pkgs: PkgCache,
}

make_repo_traits!(Repo);

impl Repo {
    pub fn new<'a, I>(id: &str, priority: i32, cpvs: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let repo_config = RepoConfig {
            priority,
            ..Default::default()
        };

        Ok(Self {
            id: id.to_string(),
            repo_config,
            pkgs: PkgCache::new(cpvs),
        })
    }

    pub fn from_path<P: AsRef<Utf8Path>>(id: &str, priority: i32, path: P) -> crate::Result<Self> {
        let path = path.as_ref();
        let data = fs::read_to_string(path).map_err(|e| Error::RepoInit(e.to_string()))?;
        let repo_config = RepoConfig {
            location: Utf8PathBuf::from(path),
            priority,
            ..Default::default()
        };
        Ok(Self {
            id: id.to_string(),
            repo_config,
            pkgs: PkgCache::new(data.lines()),
        })
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.repo_config
    }
}

impl<'a> Extend<&'a str> for Repo {
    fn extend<T: IntoIterator<Item = &'a str>>(&mut self, iter: T) {
        self.pkgs.extend(iter)
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: fake repo", self.id)
    }
}

impl PkgRepository for Repo {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type Iterator<'a> = PkgIter<'a> where Self: 'a;
    type RestrictIterator<'a> = RestrictPkgIter<'a> where Self: 'a;

    fn categories(&self) -> Vec<String> {
        self.pkgs.categories()
    }

    fn packages(&self, cat: &str) -> Vec<String> {
        self.pkgs.packages(cat)
    }

    fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
        self.pkgs.versions(cat, pkg)
    }

    fn len(&self) -> usize {
        self.pkgs.len()
    }

    fn iter(&self) -> Self::Iterator<'_> {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::RestrictIterator<'_> {
        RestrictPkgIter {
            iter: self.into_iter(),
            restrict: val.into(),
        }
    }
}

impl Repository for Repo {
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

impl<T: AsRef<Utf8Path>> Contains<T> for Repo {
    fn contains(&self, _path: T) -> bool {
        false
    }
}

impl<'a> IntoIterator for &'a Repo {
    type Item = Pkg<'a>;
    type IntoIter = PkgIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PkgIter {
            iter: self.pkgs.into_iter(),
            repo: self,
        }
    }
}

#[derive(Debug)]
pub struct PkgIter<'a> {
    iter: PkgCacheIter<'a>,
    repo: &'a Repo,
}

impl<'a> Iterator for PkgIter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|a| Pkg::new(a, self.repo))
    }
}

#[derive(Debug)]
pub struct RestrictPkgIter<'a> {
    iter: PkgIter<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for RestrictPkgIter<'a> {
    type Item = Pkg<'a>;

    #[allow(clippy::manual_find)]
    fn next(&mut self) -> Option<Self::Item> {
        for pkg in &mut self.iter {
            if self.restrict.matches(&pkg) {
                return Some(pkg);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::pkg::Package;

    use super::*;

    #[test]
    fn test_id() {
        let repo = Repo::new("fake", 0, []).unwrap();
        assert_eq!(repo.id(), "fake");
    }

    #[test]
    fn test_categories() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", 0, []).unwrap();
        assert!(repo.categories().is_empty());
        // existing pkgs
        repo = Repo::new("fake", 0, ["cat1/pkg-a-1", "cat1/pkg-b-2", "cat2/pkg-c-3"]).unwrap();
        assert_eq!(repo.categories(), ["cat1", "cat2"])
    }

    #[test]
    fn test_packages() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", 0, []).unwrap();
        assert!(repo.packages("cat").is_empty());
        // existing pkgs
        repo = Repo::new("fake", 0, ["cat1/pkg-a-1", "cat1/pkg-b-2", "cat2/pkg-c-3"]).unwrap();
        assert!(repo.packages("cat").is_empty());
        assert_eq!(repo.packages("cat1"), ["pkg-a", "pkg-b"]);
        assert_eq!(repo.packages("cat2"), ["pkg-c"]);
    }

    #[test]
    fn test_versions() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", 0, []).unwrap();
        assert!(repo.versions("cat", "pkg").is_empty());
        // existing pkgs
        repo = Repo::new("fake", 0, ["cat1/pkg-a-1", "cat2/pkg-b-1", "cat2/pkg-b-2"]).unwrap();
        assert!(repo.versions("cat", "pkg").is_empty());
        assert_eq!(repo.versions("cat1", "pkg-a"), ["1"]);
        assert_eq!(repo.versions("cat2", "pkg-b"), ["1", "2"]);
    }

    #[test]
    fn test_len() {
        let repo = Repo::new("fake", 0, []).unwrap();
        assert_eq!(repo.len(), 0);
        let repo = Repo::new("fake", 0, ["cat/pkg-0", "cat/pkg-0"]).unwrap();
        assert_eq!(repo.len(), 1);
        let repo = Repo::new("fake", 0, ["cat/pkg-0", "cat1/pkg1-1", "cat2/pkg2-2"]).unwrap();
        assert_eq!(repo.len(), 3);
    }

    #[test]
    fn test_extend() {
        let mut repo = Repo::new("fake", 0, ["cat/pkg-2"]).unwrap();
        let atoms: Vec<_> = repo
            .iter()
            .map(|pkg| format!("{}", pkg.atom().cpv()))
            .collect();
        assert_eq!(atoms, ["cat/pkg-2"]);

        // add single cpv
        repo.extend(["cat/pkg-0"]);
        let atoms: Vec<_> = repo
            .iter()
            .map(|pkg| format!("{}", pkg.atom().cpv()))
            .collect();
        assert_eq!(atoms, ["cat/pkg-0", "cat/pkg-2"]);

        // add multiple cpvs
        repo.extend(["cat/pkg-3", "cat/pkg-1", "a/b-0"]);
        let atoms: Vec<_> = repo
            .iter()
            .map(|pkg| format!("{}", pkg.atom().cpv()))
            .collect();
        assert_eq!(atoms, ["a/b-0", "cat/pkg-0", "cat/pkg-1", "cat/pkg-2", "cat/pkg-3"]);
    }

    #[test]
    fn test_contains() {
        let repo = Repo::new("fake", 0, ["cat/pkg-0"]).unwrap();

        // path containment is always false due to fake repo
        assert!(!repo.contains("cat/pkg"));

        // cpv containment
        let cpv = atom::cpv("cat/pkg-0").unwrap();
        assert!(repo.contains(&cpv));
        assert!(repo.contains(cpv));
        let cpv = atom::cpv("cat/pkg-1").unwrap();
        assert!(!repo.contains(&cpv));
        assert!(!repo.contains(cpv));

        // atom containment
        let a = Atom::from_str("cat/pkg").unwrap();
        assert!(repo.contains(&a));
        assert!(repo.contains(a));
        let a = Atom::from_str("cat/pkg-a").unwrap();
        assert!(!repo.contains(&a));
        assert!(!repo.contains(a));
    }

    #[test]
    fn test_iter() {
        let expected = ["cat/pkg-0", "acat/bpkg-1"];
        let repo = Repo::new("fake", 0, expected).unwrap();
        let atoms: Vec<_> = repo
            .iter()
            .map(|pkg| format!("{}", pkg.atom().cpv()))
            .collect();
        assert_eq!(atoms, ["acat/bpkg-1", "cat/pkg-0"]);
    }
}
