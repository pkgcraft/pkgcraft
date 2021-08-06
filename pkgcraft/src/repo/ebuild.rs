use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::repo;

#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Repo {
    pub id: String,
    pub path: PathBuf,
    cached: bool,
    #[serde(default)]
    pkgs: repo::PkgCache,
}

impl Repo {
    pub const FORMAT: &'static str = "ebuild";

    pub fn new<S: AsRef<str>>(id: S, path: S) -> Result<Repo> {
        Ok(Repo {
            id: id.as_ref().to_string(),
            path: PathBuf::from(path.as_ref()),
            ..Default::default()
        })
    }

    // TODO: build pkg cache from dir listing
    fn update_cache(&mut self) {
        self.cached = true;
    }

    pub fn from_path(id: &str, path: &str) -> Result<Self> {
        let repo_path = PathBuf::from(path);
        if !repo_path.join("profiles").exists() {
            return Err(Error::InvalidRepo {
                path: path.to_string(),
                error: "missing profiles dir".to_string(),
            });
        }

        Repo::new(id, path)
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.id, self.path.to_string_lossy())
    }
}

// TODO: fill out stub implementation
impl repo::Repo for Repo {
    fn categories(&mut self) -> repo::StringIter {
        if !self.cached {
            self.update_cache();
        }
        self.pkgs.categories()
    }

    fn packages(&mut self, cat: &str) -> repo::StringIter {
        if !self.cached {
            self.update_cache();
        }
        self.pkgs.packages(cat)
    }

    fn versions(&mut self, cat: &str, pkg: &str) -> repo::StringIter {
        if !self.cached {
            self.update_cache();
        }
        self.pkgs.versions(cat, pkg)
    }
}
