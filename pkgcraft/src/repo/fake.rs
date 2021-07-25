use std::collections::{HashMap, HashSet};
use std::fmt;
use std::iter;

use crate::atom;
use crate::error::Result;
use crate::repo;

type VersionCache = HashMap<String, HashSet<String>>;
type PkgCache = HashMap<String, VersionCache>;

#[derive(Debug, PartialEq)]
pub struct Repo {
    pub id: String,
    pkgs: PkgCache,
}


impl Repo {
    pub fn new<'a, I>(id: &str, atoms: I) -> Result<Repo>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut pkgs = PkgCache::new();
        for s in atoms.into_iter() {
            let (cat, pkg, ver) = atom::parse::cpv(s)?;
            pkgs.entry(cat.to_string()).or_insert(VersionCache::new())
                .entry(pkg.to_string()).or_insert(HashSet::new())
                .insert(ver.to_string());
        }
        Ok(Repo { id: id.to_string(), pkgs: pkgs })
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: fake repo", self.id)
    }
}

impl repo::Repo for Repo {
    fn categories(&self) -> Box<dyn Iterator<Item = &String> + '_> {
        Box::new(self.pkgs.keys())
    }

    fn packages<S: AsRef<str>>(&self, cat: S) -> Box<dyn Iterator<Item = &String> + '_> {
        match self.pkgs.get(cat.as_ref()) {
            Some(pkgs) => return Box::new(pkgs.keys()),
            None => return Box::new(iter::empty::<&String>()),
        };
    }

    fn versions<S: AsRef<str>>(&self, cat: S, pkg: S) -> Box<dyn Iterator<Item = &String> + '_> {
        match self.pkgs.get(cat.as_ref()) {
            Some(pkgs) => {
                match pkgs.get(pkg.as_ref()) {
                    Some(vers) => return Box::new(vers.iter()),
                    None => return Box::new(iter::empty::<&String>()),
                };
            },
            None => return Box::new(iter::empty::<&String>()),
        };
    }
}

#[cfg(test)]
mod tests {
    use maplit::hashset;

    use crate::repo::Repo as RepoTrait;
    use crate::macros::vec_str;

    use super::*;

    fn iter_to_set<'a>(iter: Box<dyn Iterator<Item = &'a String> + '_>) -> HashSet<&'a str> {
        iter.map(|s| s.as_str()).collect::<HashSet<&str>>()
    }

    #[test]
    fn test_categories() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", []).unwrap();
        assert_eq!(iter_to_set(repo.categories()), hashset!{});
        // existing pkgs
        repo = Repo::new("fake", ["cat1/pkg-a-1", "cat1/pkg-b-2", "cat2/pkg-c-3"]).unwrap();
        assert_eq!(iter_to_set(repo.categories()), hashset!{"cat1", "cat2"});
    }

    #[test]
    fn test_packages() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", []).unwrap();
        assert_eq!(iter_to_set(repo.packages("cat")), hashset!{});
        // existing pkgs
        repo = Repo::new("fake", ["cat1/pkg-a-1", "cat1/pkg-b-2", "cat2/pkg-c-3"]).unwrap();
        assert_eq!(iter_to_set(repo.packages("cat")), hashset!{});
        assert_eq!(iter_to_set(repo.packages("cat1")), hashset!{"pkg-a", "pkg-b"});
        assert_eq!(iter_to_set(repo.packages("cat2")), hashset!{"pkg-c"});
    }

    #[test]
    fn test_versions() {
        let mut repo: Repo;
        // empty repo
        repo = Repo::new("fake", []).unwrap();
        assert_eq!(iter_to_set(repo.versions("cat", "pkg")), hashset!{});
        // existing pkgs
        repo = Repo::new("fake", ["cat1/pkg-a-1", "cat2/pkg-b-1", "cat2/pkg-b-2"]).unwrap();
        assert_eq!(iter_to_set(repo.versions("cat", "pkg")), hashset!{});
        assert_eq!(iter_to_set(repo.versions("cat1", "pkg-a")), hashset!{"1"});
        assert_eq!(iter_to_set(repo.versions("cat2", "pkg-b")), hashset!{"1", "2"});
    }
}
