use std::fmt;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::repo;

#[derive(Debug, Default)]
pub struct Repo {
    pub id: String,
    pub path: PathBuf,
    cached: bool,
    pkgs: repo::PkgCache,
}

impl Repo {
    pub const FORMAT: &'static str = "ebuild";

    pub fn new<P: AsRef<Path>>(id: &str, path: P) -> Result<Self> {
        Ok(Repo {
            id: id.to_string(),
            path: PathBuf::from(path.as_ref()),
            ..Default::default()
        })
    }

    // TODO: build pkg cache from dir listing
    fn update_cache(&mut self) {
        self.cached = true;
    }

    pub fn from_path<P: AsRef<Path>>(id: &str, path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.join("profiles").exists() {
            return Err(Error::InvalidRepo {
                path: PathBuf::from(path),
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
