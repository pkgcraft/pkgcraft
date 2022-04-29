use std::fmt;
use std::fs;
use std::iter;
use std::path::Path;

use indexmap::IndexSet;

use crate::pkg::Pkg;
use crate::{atom, repo, Error, Result};

#[derive(Debug, Default)]
pub struct Repo {
    id: String,
    pkgs: repo::PkgCache,
}

impl Repo {
    pub(super) const FORMAT: &'static str = "fake";

    fn new<'a, I>(id: &str, atoms: I) -> Result<Repo>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut pkgmap = repo::PkgMap::new();
        let mut cpvs = Vec::<atom::Atom>::new();
        for s in atoms.into_iter() {
            cpvs.push(atom::parse::cpv(s)?);
        }

        cpvs.sort();

        for cpv in cpvs {
            pkgmap
                .entry(cpv.category)
                .or_insert_with(repo::VersionMap::new)
                .entry(cpv.package)
                .or_insert_with(IndexSet::new)
                .insert(cpv.version.unwrap().to_string());
        }

        let pkgs = repo::PkgCache { pkgmap };
        Ok(Repo {
            id: id.to_string(),
            pkgs,
        })
    }

    pub(super) fn from_path<P: AsRef<Path>>(id: &str, path: P) -> Result<Self> {
        let data = fs::read_to_string(path.as_ref()).map_err(|e| Error::RepoInit(e.to_string()))?;
        Repo::new(id, data.lines())
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: fake repo", self.id)
    }
}

impl repo::Repo for Repo {
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

    fn iter(&self) -> Box<dyn Iterator<Item = Box<dyn Pkg>>> {
        Box::new(iter::empty::<Box<dyn Pkg>>())
    }

    fn len(&self) -> usize {
        self.pkgs.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::repo::Repo as RepoTrait;

    use super::*;

    #[test]
    fn test_id() {
        let repo = Repo::new("fake", []).unwrap();
        assert_eq!(repo.id(), "fake");
    }

    #[test]
    fn test_categories() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", []).unwrap();
        assert_eq!(repo.categories(), Vec::<String>::new());
        // existing pkgs
        repo = Repo::new("fake", ["cat1/pkg-a-1", "cat1/pkg-b-2", "cat2/pkg-c-3"]).unwrap();
        assert_eq!(repo.categories(), ["cat1", "cat2"])
    }

    #[test]
    fn test_packages() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", []).unwrap();
        assert_eq!(repo.packages("cat"), Vec::<String>::new());
        // existing pkgs
        repo = Repo::new("fake", ["cat1/pkg-a-1", "cat1/pkg-b-2", "cat2/pkg-c-3"]).unwrap();
        assert_eq!(repo.packages("cat"), Vec::<String>::new());
        assert_eq!(repo.packages("cat1"), ["pkg-a", "pkg-b"]);
        assert_eq!(repo.packages("cat2"), ["pkg-c"]);
    }

    #[test]
    fn test_versions() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", []).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), Vec::<String>::new());
        // existing pkgs
        repo = Repo::new("fake", ["cat1/pkg-a-1", "cat2/pkg-b-1", "cat2/pkg-b-2"]).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), Vec::<String>::new());
        assert_eq!(repo.versions("cat1", "pkg-a"), ["1"]);
        assert_eq!(repo.versions("cat2", "pkg-b"), ["1", "2"]);
    }

    #[test]
    fn test_len() {
        let repo = Repo::new("fake", []).unwrap();
        assert_eq!(repo.len(), 0);
        let repo = Repo::new("fake", ["cat/pkg-0", "cat/pkg-0"]).unwrap();
        assert_eq!(repo.len(), 1);
        let repo = Repo::new("fake", ["cat/pkg-0", "cat1/pkg1-1", "cat2/pkg2-2"]).unwrap();
        assert_eq!(repo.len(), 3);
    }
}
