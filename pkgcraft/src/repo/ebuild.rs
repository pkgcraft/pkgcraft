use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::eapi;
use crate::error::Error;
use crate::repo;

#[derive(Debug, Default)]
pub struct Repo {
    id: String,
    path: PathBuf,
    cached: bool,
    pkgs: repo::PkgCache,
}

impl Repo {
    pub const FORMAT: &'static str = "ebuild";

    pub fn new<P: AsRef<Path>>(id: &str, path: P) -> Result<Self, Error> {
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

    pub fn from_path<P: AsRef<Path>>(id: &str, path: P) -> Result<Self, Error> {
        let path = path.as_ref();
        if !path.join("profiles").exists() {
            return Err(Error::RepoInvalid {
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

#[derive(Debug)]
pub struct TempRepo {
    tempdir: TempDir,
    repo: Repo,
}

impl TempRepo {
    pub fn new<P: AsRef<Path>>(
        id: &str,
        path: Option<P>,
        eapi: Option<&eapi::Eapi>,
    ) -> Result<Self, Error> {
        let path = match path {
            Some(p) => PathBuf::from(p.as_ref()),
            None => env::temp_dir(),
        };
        let eapi = format!("{}", eapi.unwrap_or(eapi::EAPI_LATEST));
        let tempdir = TempDir::new_in(path)
            .map_err(|e| Error::RepoInit(format!("failed creating temp repo {:?}: {}", id, e)))?;
        let temp_path = tempdir.path();

        for dir in ["metadata", "profiles"] {
            fs::create_dir(temp_path.join(dir)).map_err(|e| {
                Error::RepoInit(format!("failed creating temp repo {:?}: {}", id, e))
            })?;
        }
        fs::write(temp_path.join("profiles/repo_name"), format!("{}\n", id))
            .map_err(|e| Error::RepoInit(format!("failed writing temp repo id: {}", e)))?;
        fs::write(temp_path.join("profiles/eapi"), format!("{}\n", eapi))
            .map_err(|e| Error::RepoInit(format!("failed writing temp repo EAPI: {}", e)))?;

        let repo = Repo::from_path(id, temp_path)?;
        Ok(TempRepo { tempdir, repo })
    }

    pub fn persist(self) -> PathBuf {
        self.tempdir.into_path()
    }
}

impl fmt::Display for TempRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.repo)
    }
}

impl repo::Repo for TempRepo {
    #[inline]
    fn categories(&mut self) -> repo::StringIter {
        self.repo.categories()
    }

    #[inline]
    fn packages(&mut self, cat: &str) -> repo::StringIter {
        self.repo.packages(cat)
    }

    #[inline]
    fn versions(&mut self, cat: &str, pkg: &str) -> repo::StringIter {
        self.repo.versions(cat, pkg)
    }
}
