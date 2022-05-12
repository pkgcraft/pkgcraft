use std::fmt;
use std::fs;
use std::path::Path;

use indexmap::IndexSet;

use crate::{atom, pkg, repo, Error, Result};

#[derive(Debug, Default, PartialEq, Eq)]
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
        let mut cpvs = IndexSet::<atom::Atom>::new();
        for s in atoms.into_iter() {
            cpvs.insert(atom::parse::cpv(s)?);
        }

        cpvs.sort();

        for cpv in &cpvs {
            pkgmap
                .entry(cpv.category().into())
                .or_insert_with(repo::VersionMap::new)
                .entry(cpv.package().into())
                .or_insert_with(IndexSet::new)
                .insert(cpv.version().unwrap().into());
        }

        let pkgs = repo::PkgCache {
            pkgmap,
            atoms: cpvs,
        };
        Ok(Repo {
            id: id.to_string(),
            pkgs,
        })
    }

    pub(super) fn from_path<P: AsRef<Path>>(id: &str, path: P) -> Result<Self> {
        let data = fs::read_to_string(path.as_ref()).map_err(|e| Error::RepoInit(e.to_string()))?;
        Repo::new(id, data.lines())
    }

    pub fn iter(&self) -> PkgIter {
        self.into_iter()
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

    fn len(&self) -> usize {
        self.pkgs.len()
    }

    fn is_empty(&self) -> bool {
        self.pkgs.is_empty()
    }
}

impl<T: AsRef<Path>> repo::Contains<T> for Repo {
    fn contains(&self, _path: T) -> bool {
        false
    }
}

impl repo::Contains<&atom::Atom> for Repo {
    fn contains(&self, atom: &atom::Atom) -> bool {
        self.pkgs.atoms.contains(atom)
    }
}

impl repo::Contains<atom::Atom> for Repo {
    fn contains(&self, atom: atom::Atom) -> bool {
        self.pkgs.atoms.contains(&atom)
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

#[cfg(test)]
mod tests {
    use crate::atom;
    use crate::repo::{Contains, Repo as RepoTrait};

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

    #[test]
    fn test_contains() {
        let repo = Repo::new("fake", ["cat/pkg-0"]).unwrap();
        // path containment is always false due to fake repo
        assert!(!repo.contains("cat/pkg"));
        // atom containment
        let cpv = atom::parse::cpv("cat/pkg-0").unwrap();
        assert!(repo.contains(&cpv));
        assert!(repo.contains(cpv));
    }

    #[test]
    fn test_into_iter() {
        let expected = ["cat/pkg-0", "acat/bpkg-1"];
        let repo = Repo::new("fake", expected).unwrap();
        let atoms: Vec<String> = repo.into_iter().map(|a| format!("{}", a)).collect();
        assert_eq!(atoms, ["acat/bpkg-1", "cat/pkg-0"]);
    }
}
