use std::cmp::Ordering;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use itertools::{Either, Itertools};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use tracing::warn;

use crate::eapi::Eapi;
use crate::repo::ebuild_temp::Repo as TempRepo;
use crate::repo::set::RepoSet;
use crate::repo::{Repo, RepoFormat, Repository};
use crate::sync::Syncer;
use crate::Error;

use super::Repos;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct RepoConfig {
    pub(crate) location: Utf8PathBuf,
    #[serde_as(as = "DisplayFromStr")]
    pub(crate) format: RepoFormat,
    pub(crate) priority: i32,
    pub(crate) sync: Option<Syncer>,
}

impl RepoConfig {
    fn new<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        let path = path.as_ref();
        let data = fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("failed loading repo config {path:?}: {e}")))?;

        let config: RepoConfig = toml::from_str(&data)
            .map_err(|e| Error::Config(format!("failed loading repo config toml {path:?}: {e}")))?;

        Ok(config)
    }

    pub(crate) fn sync(&self) -> crate::Result<()> {
        match &self.sync {
            Some(syncer) => syncer.sync(&self.location),
            None => Ok(()),
        }
    }
}

impl PartialOrd for RepoConfig {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => Some(self.location.cmp(&other.location)),
            cmp => Some(cmp),
        }
    }
}

impl Ord for RepoConfig {
    fn cmp(&self, other: &Self) -> Ordering {
        // unwrap the partial ordering result since they're always orderable
        self.partial_cmp(other).unwrap()
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Config {
    config_dir: Utf8PathBuf,
    repo_dir: Utf8PathBuf,
    #[serde(skip)]
    repos: IndexMap<String, Repo>,
}

impl Config {
    pub(super) fn new(config_dir: &Utf8Path, db_dir: &Utf8Path) -> crate::Result<Self> {
        let config_dir = config_dir.join("repos");
        let repo_dir = db_dir.join("repos");

        let mut configs = vec![];
        if config_dir.exists() {
            let entries = fs::read_dir(&config_dir).map_err(|e| Error::Config(e.to_string()))?;

            for entry in entries {
                let p = entry.map_err(|e| Error::Config(e.to_string()))?.path();
                if p.is_file() {
                    if let Some(name) = p
                        .file_name()
                        .and_then(|p| p.to_str().map(|s| s.to_string()))
                        .filter(|s| !s.starts_with('.'))
                    {
                        // ignore bad configs
                        match RepoConfig::new(&p) {
                            Ok(config) => {
                                configs.push((name, config));
                            }
                            Err(err) => warn!("{err}"),
                        }
                    }
                }
            }
        }

        // load configured repos
        let mut repos = vec![];
        for (name, c) in configs.into_iter() {
            // ignore unsynced or nonexistent repos
            match c
                .format
                .load_from_path(&name, c.priority, &c.location, false)
            {
                Ok(repo) => repos.push(repo),
                Err(err) => warn!("{err}"),
            }
        }

        let mut config = Self {
            config_dir,
            repo_dir,
            repos: Default::default(),
        };

        // finalize, sort, and add repos to the config
        config.extend(&repos, false)?;
        Ok(config)
    }

    /// Create related repo config paths.
    pub(super) fn create_paths(&self) -> crate::Result<()> {
        for path in [&self.config_dir, &self.repo_dir] {
            fs::create_dir_all(path).map_err(|e| Error::Config(e.to_string()))?;
        }
        Ok(())
    }

    /// Add external repo from a URI.
    pub(super) fn add_uri(&mut self, name: &str, priority: i32, uri: &str) -> crate::Result<Repo> {
        let config = RepoConfig {
            location: self.repo_dir.join(name),
            priority,
            sync: Some(Syncer::from_str(uri)?),
            ..Default::default()
        };
        config.sync()?;

        let repo = Repo::from_path(name, priority, config.location, false)?;

        // write repo config file to disk
        let data = toml::to_string(repo.repo_config())
            .map_err(|e| Error::Config(format!("failed serializing repo config to toml: {e}")))?;
        let path = self.config_dir.join(name);
        let mut file = fs::File::create(&path).map_err(|e| {
            Error::Config(format!("failed creating repo config file: {path:?}: {e}"))
        })?;
        file.write_all(data.as_bytes()).map_err(|e| {
            Error::Config(format!("failed writing repo config file: {path:?}: {e}"))
        })?;

        Ok(repo)
    }

    pub(super) fn create(&mut self, name: &str, priority: i32) -> crate::Result<Repo> {
        let path = self.repo_dir.join(name);
        // create temporary repo and persist it to disk
        let temp_repo = TempRepo::new(name, Some(&self.repo_dir), priority, None)?;
        temp_repo.persist(Some(&path))?;
        Repo::from_path(name, priority, path.as_str(), false)
    }

    pub(super) fn create_temp(
        &mut self,
        name: &str,
        priority: i32,
        eapi: Option<&Eapi>,
    ) -> crate::Result<TempRepo> {
        TempRepo::new(name, None, priority, eapi)
    }

    pub(super) fn del<S: AsRef<str>>(&mut self, repos: &[S], clean: bool) -> crate::Result<()> {
        for name in repos {
            let name = name.as_ref();
            // error out if repo config is missing
            // physical repo files are allowed to be missing
            if let Some(repo) = self.repos.get(name) {
                if clean {
                    fs::remove_dir_all(repo.path()).map_err(|e| {
                        Error::Config(format!("failed removing repo files: {:?}: {e}", repo.path()))
                    })?;
                    let path = self.config_dir.join(name);
                    fs::remove_file(&path).map_err(|e| {
                        Error::Config(format!("failed removing repo config: {path:?}: {e}"))
                    })?;
                }
                self.repos.shift_remove(name as &str);
            }
        }
        Ok(())
    }

    // TODO: add concurrent syncing support with output progress
    pub fn sync<S: AsRef<str>>(&self, repos: Vec<S>) -> crate::Result<()> {
        let repos: Vec<_> = match &repos {
            names if !names.is_empty() => names.iter().map(|s| s.as_ref()).collect(),
            // sync all configured repos if none were passed
            _ => self.repos.keys().map(|s| s.as_str()).collect(),
        };

        let mut failed = vec![];
        for name in repos {
            if let Some(repo) = self.repos.get(name) {
                if let Err(e) = repo.sync() {
                    failed.push((name, e));
                }
            }
        }

        if failed.is_empty() {
            Ok(())
        } else {
            let errors = failed
                .iter()
                .map(|(name, e)| format!("{name}: {e}"))
                .join("\n\t");
            Err(Error::Config(format!("failed syncing:\n\t{errors}")))
        }
    }

    /// RepoSet objects from sets of repos registered in the config object.
    pub fn set(&self, kind: Repos) -> RepoSet {
        let repos = self.repos.values();
        match kind {
            Repos::All => RepoSet::new(repos),
            Repos::Ebuild => RepoSet::new(repos.filter(|r| matches!(r, Repo::Ebuild(_)))),
        }
    }

    /// Get a configured repo.
    pub fn get(&self, key: &str) -> Option<&Repo> {
        self.repos.get(key)
    }

    /// Returns true if no repos exist and false otherwise.
    pub(super) fn is_empty(&self) -> bool {
        self.repos.is_empty()
    }

    /// Extend the config with multiple repos.
    pub(super) fn extend<'a, I: IntoIterator<Item = &'a Repo>>(
        &mut self,
        repos: I,
        external: bool,
    ) -> crate::Result<()> {
        // check for existing repos since duplicate repo names aren't allowed
        let (mut existing, repos): (Vec<_>, Vec<_>) = repos.into_iter().partition_map(|r| {
            // use path names for external repos
            let path = r.path().as_str();
            let name = if external && !path.is_empty() {
                path
            } else {
                r.name()
            };

            if self.repos.get(name).is_some() {
                Either::Left(r)
            } else {
                Either::Right((name, r))
            }
        });

        if !existing.is_empty() {
            existing.sort();
            let existing = existing.iter().map(|r| r.id()).join(", ");
            return Err(Error::Config(format!("can't override existing repos: {existing}")));
        }

        // copy original repos so it can be reverted to if an error occurs
        let orig_repos = self.repos.clone();

        // add repos to config
        self.repos
            .extend(repos.iter().map(|(s, r)| (s.to_string(), (*r).clone())));

        // verify new repos
        for (_, repo) in &repos {
            if let Err(e) = repo.finalize(&self.repos) {
                // revert to previous repos
                self.repos = orig_repos;
                return Err(e);
            }
        }

        self.sort();
        Ok(())
    }

    pub fn iter(&self) -> ReposIter<'_> {
        self.into_iter()
    }

    /// Sort repos by priority then by name.
    fn sort(&mut self) {
        self.repos.sort_by(|_k1, v1, _k2, v2| v1.cmp(v2));
    }
}

pub struct ReposIter<'a> {
    iter: indexmap::map::Iter<'a, String, Repo>,
}

impl<'a> IntoIterator for &'a Config {
    type Item = (&'a str, &'a Repo);
    type IntoIter = ReposIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ReposIter { iter: self.repos.iter() }
    }
}

impl<'a> Iterator for ReposIter<'a> {
    type Item = (&'a str, &'a Repo);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(id, repo)| (id.as_str(), repo))
    }
}
