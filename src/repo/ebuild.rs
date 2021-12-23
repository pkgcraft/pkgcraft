use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::eapi;
use crate::error::Error;
use crate::repo;

#[derive(Debug, Default)]
pub(crate) struct Repo {
    id: String,
    path: PathBuf,
    pkgs: repo::PkgCache,
}

impl Repo {
    pub(super) const FORMAT: &'static str = "ebuild";

    fn new<P: AsRef<Path>>(id: &str, path: P) -> crate::Result<Self> {
        Ok(Repo {
            id: id.to_string(),
            path: PathBuf::from(path.as_ref()),
            ..Default::default()
        })
    }

    pub(super) fn from_path<P: AsRef<Path>>(id: &str, path: P) -> crate::Result<Self> {
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
        self.pkgs.categories()
    }

    fn packages(&mut self, cat: &str) -> repo::StringIter {
        self.pkgs.packages(cat)
    }

    fn versions(&mut self, cat: &str, pkg: &str) -> repo::StringIter {
        self.pkgs.versions(cat, pkg)
    }
}

/// A temporary repo that is automatically deleted when it goes out of scope.
#[derive(Debug)]
pub(crate) struct TempRepo {
    tempdir: TempDir,
    repo: Repo,
}

impl TempRepo {
    /// Attempts to create a temporary repo inside an optional path or inside `env::temp_dir()` if
    /// no path is specified.
    pub(crate) fn new<P: AsRef<Path>>(
        id: &str,
        path: Option<P>,
        eapi: Option<&eapi::Eapi>,
    ) -> crate::Result<Self> {
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

    /// Attempts to persist the temporary repo to disk, returning the [`PathBuf`] where it is
    /// located.
    pub(crate) fn persist(self, path: Option<&PathBuf>) -> crate::Result<PathBuf> {
        let mut repo_path = self.tempdir.into_path();
        if let Some(path) = path {
            fs::rename(&repo_path, path).map_err(|e| {
                Error::RepoInit(format!(
                    "failed renaming repo: {:?} -> {:?}: {}",
                    &repo_path, &path, e
                ))
            })?;
            repo_path = path.clone();
        }
        Ok(repo_path)
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
