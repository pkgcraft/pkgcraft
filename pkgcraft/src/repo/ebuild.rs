use std::error::Error;
use std::fmt;
use std::iter;

use crate::repo;

#[derive(Debug, PartialEq)]
pub struct Repo {
    pub id: String,
    pub path: String,
    pkgs: repo::PkgCache,
}

impl Repo {
    pub fn new(id: &str, path: &str) -> Result<Repo, Box<dyn Error>> {
        Ok(Repo {
            id: id.to_string(),
            path: path.to_string(),
            pkgs: repo::PkgCache::new(),
        })
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.id, self.path)
    }
}

// TODO: fill out stub implementation
impl repo::Repo for Repo {
    fn categories(&self) -> Box<dyn Iterator<Item = &String> + '_> {
        Box::new(iter::empty::<&String>())
    }

    fn packages<S: AsRef<str>>(&self, cat: S) -> Box<dyn Iterator<Item = &String> + '_> {
        Box::new(iter::empty::<&String>())
    }

    fn versions<S: AsRef<str>>(&self, cat: S, pkg: S) -> Box<dyn Iterator<Item = &String> + '_> {
        Box::new(iter::empty::<&String>())
    }
}
