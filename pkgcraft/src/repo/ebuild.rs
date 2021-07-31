use std::fmt;
use std::iter;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::repo;

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Repo {
    pub id: String,
    pub path: String,
    #[serde(default)] // https://github.com/mehcode/config-rs/issues/114
    pkgs: repo::PkgCache,
}

impl Repo {
    pub fn new<S: AsRef<str>>(id: S, path: S) -> Result<Repo> {
        Ok(Repo {
            id: id.as_ref().to_string(),
            path: path.as_ref().to_string(),
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

    fn from_path<S: AsRef<str>>(id: S, path: S) -> Result<Self> {
        let id = id.as_ref();
        let path = path.as_ref();
        let error: String;

        let repo_path = PathBuf::from(path);
        if repo_path.join("profiles").exists() {
            return Repo::new(id, path);
        } else {
            error = format!("missing profiles dir"); 
        }

        Err(Error::InvalidRepo { path: path.to_string(), error })
    }
}
