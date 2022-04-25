use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, fmt, fs, io};

use ini::Ini;
use tempfile::TempDir;

use crate::config::Config;
use crate::{eapi, repo, Error, Result};

const DEFAULT_SECTION: Option<String> = None;

pub(crate) struct Metadata {
    path: Option<PathBuf>,
    ini: Ini,
}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let section = self.ini.section(DEFAULT_SECTION);
        f.debug_struct("Metadata")
            .field("path", &self.path)
            .field("ini", &section)
            .finish()
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            path: None,
            ini: Ini::new(),
        }
    }
}

impl Metadata {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        match Ini::load_from_file(path) {
            Ok(ini) => Ok(Metadata {
                path: Some(PathBuf::from(path)),
                ini,
            }),
            Err(ini::Error::Io(e)) if e.kind() == io::ErrorKind::NotFound => Ok(Metadata {
                path: Some(PathBuf::from(path)),
                ini: Ini::new(),
            }),
            Err(e) => Err(Error::InvalidValue(format!("invalid repo layout: {path:?}: {e}"))),
        }
    }

    #[cfg(test)]
    pub(crate) fn set<S1, S2>(&mut self, key: S1, val: S2)
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        self.ini.set_to(DEFAULT_SECTION, key.into(), val.into());
    }

    #[cfg(test)]
    pub(crate) fn write(&self) -> Result<()> {
        match &self.path {
            Some(path) => self
                .ini
                .write_to_file(path)
                .map_err(|e| Error::IO(e.to_string())),
            None => Ok(()),
        }
    }

    fn get_list<S: AsRef<str>>(&self, key: S) -> Vec<String> {
        match self.ini.get_from(DEFAULT_SECTION, key.as_ref()) {
            None => vec![],
            Some(s) => s.split_whitespace().map(|s| s.into()).collect(),
        }
    }

    pub(crate) fn masters(&self) -> Vec<String> {
        self.get_list("masters")
    }
}

#[derive(Debug, Default)]
pub struct Repo {
    id: String,
    pub(super) path: PathBuf,
    pub(super) config: Metadata,
    pkgs: repo::PkgCache,
}

impl Repo {
    pub(super) const FORMAT: &'static str = "ebuild";

    fn new<S, P>(id: S, path: P, config: Metadata) -> Result<Self>
    where
        S: AsRef<str>,
        P: AsRef<Path>,
    {
        Ok(Repo {
            id: id.as_ref().to_string(),
            path: PathBuf::from(path.as_ref()),
            config,
            ..Default::default()
        })
    }

    pub(super) fn from_path<S, P>(id: S, path: P) -> Result<Self>
    where
        S: AsRef<str>,
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let profiles_base = path.join("profiles");

        if !profiles_base.exists() {
            return Err(Error::InvalidRepo {
                path: PathBuf::from(path),
                error: "missing profiles dir".to_string(),
            });
        }

        let config =
            Metadata::new(path.join("metadata/layout.conf")).map_err(|e| Error::InvalidRepo {
                path: PathBuf::from(path),
                error: e.to_string(),
            })?;
        Repo::new(id, path, config)
    }

    pub fn masters(&self) -> Result<Vec<Arc<repo::Repository>>> {
        let config = Config::current();
        let mut masters = vec![];
        for id in self.config.masters() {
            match config.repos.repos.get(&id) {
                Some(r) => masters.push(r.clone()),
                None => {
                    return Err(Error::InvalidRepo {
                        path: self.path.clone(),
                        error: format!("nonexistent master: {id}"),
                    })
                }
            }
        }
        Ok(masters)
    }

    pub fn trees(&self) -> Result<Vec<Arc<repo::Repository>>> {
        let mut trees = self.masters()?;
        let config = Config::current();
        match config.repos.repos.get(&self.id) {
            Some(r) => trees.push(r.clone()),
            None => {
                return Err(Error::InvalidRepo {
                    path: self.path.clone(),
                    error: format!("unconfigured repo: {}", self.id),
                })
            }
        }
        Ok(trees)
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

    fn id(&self) -> &str {
        &self.id
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
    ) -> Result<Self> {
        let path = match path {
            Some(p) => PathBuf::from(p.as_ref()),
            None => env::temp_dir(),
        };
        let eapi = format!("{}", eapi.unwrap_or(eapi::EAPI_LATEST));
        let tempdir = TempDir::new_in(path)
            .map_err(|e| Error::RepoInit(format!("failed creating temp repo {id:?}: {e}")))?;
        let temp_path = tempdir.path();

        for dir in ["metadata", "profiles"] {
            fs::create_dir(temp_path.join(dir))
                .map_err(|e| Error::RepoInit(format!("failed creating temp repo {id:?}: {e}")))?;
        }
        fs::write(temp_path.join("profiles/repo_name"), format!("{id}\n"))
            .map_err(|e| Error::RepoInit(format!("failed writing temp repo id: {e}")))?;
        fs::write(temp_path.join("profiles/eapi"), format!("{eapi}\n"))
            .map_err(|e| Error::RepoInit(format!("failed writing temp repo EAPI: {e}")))?;

        let repo = Repo::from_path(id, temp_path)?;
        Ok(TempRepo { tempdir, repo })
    }

    /// Attempts to persist the temporary repo to disk, returning the [`PathBuf`] where it is
    /// located.
    pub(crate) fn persist<P: AsRef<Path>>(self, path: Option<P>) -> Result<PathBuf> {
        let mut repo_path = self.tempdir.into_path();
        if let Some(path) = path {
            let path = path.as_ref();
            fs::rename(&repo_path, path).map_err(|e| {
                Error::RepoInit(format!("failed renaming repo: {repo_path:?} -> {path:?}: {e}"))
            })?;
            repo_path = path.into();
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

    #[inline]
    fn id(&self) -> &str {
        &self.repo.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let temprepo = TempRepo::new("test", None::<&str>, None).unwrap();
        let mut repo = temprepo.repo;
        repo.config.set("masters", "a b c");
        repo.config.write().unwrap();
        let test_repo = Repo::from_path(repo.id, repo.path).unwrap();
        assert_eq!(test_repo.config.masters(), ["a", "b", "c"]);
    }
}
