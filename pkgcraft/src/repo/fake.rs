use std::collections::HashSet;
use std::fmt;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::atom;
use crate::error::Result;
use crate::repo;

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Repo {
    pub id: String,
    #[serde(default)]
    pkgs: repo::PkgCache,
}

impl Repo {
    pub const FORMAT: &'static str = "fake";

    pub fn new<'a, I>(id: &str, atoms: I) -> Result<Repo>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut pkgmap = repo::PkgMap::new();
        for s in atoms.into_iter() {
            let (cat, pkg, ver) = atom::parse::cpv(s)?;
            pkgmap
                .entry(cat.to_string())
                .or_insert_with(repo::VersionMap::new)
                .entry(pkg.to_string())
                .or_insert_with(HashSet::new)
                .insert(ver.to_string());
        }

        let pkgs = repo::PkgCache { pkgmap };
        Ok(Repo {
            id: id.to_string(),
            pkgs,
        })
    }

    pub fn from_path(id: &str, path: &str) -> Result<Self> {
        let data = fs::read_to_string(path)?;
        Repo::new(id, data.lines())
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: fake repo", self.id)
    }
}

impl repo::Repo for Repo {
    fn categories(&mut self) -> repo::StringIter {
        self.pkgs.categories()
    }

    fn packages(&mut self, cat: &str) -> repo::StringIter {
        self.pkgs.packages(cat)
    }

    fn versions(&mut self, cat: &str, pkg: &str) -> repo::StringIter {
        self.pkgs.versions(cat, pkg)
    }
}

#[cfg(test)]
mod tests {
    use maplit::hashset;

    use crate::repo::Repo as RepoTrait;

    use super::*;

    fn iter_to_set<'a>(iter: Box<dyn Iterator<Item = &'a String> + '_>) -> HashSet<&'a str> {
        iter.map(|s| s.as_str()).collect::<HashSet<&str>>()
    }

    #[test]
    fn test_categories() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", []).unwrap();
        assert_eq!(iter_to_set(repo.categories()), hashset! {});
        // existing pkgs
        repo = Repo::new("fake", ["cat1/pkg-a-1", "cat1/pkg-b-2", "cat2/pkg-c-3"]).unwrap();
        assert_eq!(iter_to_set(repo.categories()), hashset! {"cat1", "cat2"});
    }

    #[test]
    fn test_packages() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", []).unwrap();
        assert_eq!(iter_to_set(repo.packages("cat")), hashset! {});
        // existing pkgs
        repo = Repo::new("fake", ["cat1/pkg-a-1", "cat1/pkg-b-2", "cat2/pkg-c-3"]).unwrap();
        assert_eq!(iter_to_set(repo.packages("cat")), hashset! {});
        assert_eq!(
            iter_to_set(repo.packages("cat1")),
            hashset! {"pkg-a", "pkg-b"}
        );
        assert_eq!(iter_to_set(repo.packages("cat2")), hashset! {"pkg-c"});
    }

    #[test]
    fn test_versions() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", []).unwrap();
        assert_eq!(iter_to_set(repo.versions("cat", "pkg")), hashset! {});
        // existing pkgs
        repo = Repo::new("fake", ["cat1/pkg-a-1", "cat2/pkg-b-1", "cat2/pkg-b-2"]).unwrap();
        assert_eq!(iter_to_set(repo.versions("cat", "pkg")), hashset! {});
        assert_eq!(iter_to_set(repo.versions("cat1", "pkg-a")), hashset! {"1"});
        assert_eq!(
            iter_to_set(repo.versions("cat2", "pkg-b")),
            hashset! {"1", "2"}
        );
    }
}
