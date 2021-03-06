use std::fmt;
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

use super::{make_repo_traits, Repository};
use crate::config::RepoConfig;
use crate::pkg::Package;
use crate::restrict::{Restrict, Restriction};
use crate::{atom, pkg, repo, Error};

#[derive(Debug, Default)]
pub struct Repo {
    id: String,
    repo_config: RepoConfig,
    pkgs: repo::PkgCache,
}

make_repo_traits!(Repo);

impl Repo {
    #[cfg(test)]
    pub(crate) fn new<'a, I>(id: &str, priority: i32, atoms: I) -> crate::Result<Repo>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let repo_config = RepoConfig {
            priority,
            ..Default::default()
        };

        // TODO: replace from_iter() usage with try_from_iter()
        Ok(Repo {
            id: id.to_string(),
            repo_config,
            pkgs: repo::PkgCache::from_iter(atoms),
        })
    }

    pub(super) fn from_path<P: AsRef<Utf8Path>>(
        id: &str,
        priority: i32,
        path: P,
    ) -> crate::Result<Self> {
        let path = path.as_ref();
        let data = fs::read_to_string(path).map_err(|e| Error::RepoInit(e.to_string()))?;
        let repo_config = RepoConfig {
            location: Utf8PathBuf::from(path),
            priority,
            ..Default::default()
        };
        Ok(Repo {
            id: id.to_string(),
            repo_config,
            pkgs: repo::PkgCache::from_iter(data.lines()),
        })
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.repo_config
    }

    pub fn iter(&self) -> PkgIter {
        self.into_iter()
    }

    pub fn iter_restrict<T: Into<Restrict>>(&self, val: T) -> RestrictPkgIter {
        RestrictPkgIter {
            iter: self.into_iter(),
            restrict: val.into(),
        }
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: fake repo", self.id)
    }
}

impl Repository for Repo {
    fn categories(&self) -> Vec<String> {
        self.pkgs.categories()
    }

    fn packages(&self, cat: &str) -> Vec<String> {
        self.pkgs.packages(cat)
    }

    fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
        self.pkgs.versions(cat, pkg)
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

    fn len(&self) -> usize {
        self.pkgs.len()
    }

    fn is_empty(&self) -> bool {
        self.pkgs.is_empty()
    }
}

impl<T: AsRef<Utf8Path>> repo::Contains<T> for Repo {
    fn contains(&self, _path: T) -> bool {
        false
    }
}

impl<'a> IntoIterator for &'a Repo {
    type Item = pkg::fake::Pkg<'a>;
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
    iter: repo::PkgCacheIter<'a>,
    repo: &'a Repo,
}

impl<'a> Iterator for PkgIter<'a> {
    type Item = pkg::fake::Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|a| pkg::fake::Pkg::new(a, self.repo))
    }
}

#[derive(Debug)]
pub struct RestrictPkgIter<'a> {
    iter: PkgIter<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for RestrictPkgIter<'a> {
    type Item = pkg::fake::Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(p) => {
                    if self.restrict.matches(&p) {
                        return Some(p);
                    }
                }
                None => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::atom;
    use crate::repo::{Contains, Repository};

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
        let a = atom::Atom::from_str("cat/pkg").unwrap();
        assert!(repo.contains(&a));
        assert!(repo.contains(a));
        let a = atom::Atom::from_str("cat/pkg-a").unwrap();
        assert!(!repo.contains(&a));
        assert!(!repo.contains(a));
    }

    #[test]
    fn test_iter() {
        let expected = ["cat/pkg-0", "acat/bpkg-1"];
        let repo = Repo::new("fake", 0, expected).unwrap();
        let atoms: Vec<_> = repo.iter().map(|a| format!("{a}")).collect();
        assert_eq!(atoms, ["acat/bpkg-1::fake", "cat/pkg-0::fake"]);
    }
}
