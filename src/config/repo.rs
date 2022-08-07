use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::repo::ebuild::TempRepo;
use crate::repo::{Repo, Repository};
use crate::sync::Syncer;
use crate::Error;

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct RepoConfig {
    pub(crate) location: Utf8PathBuf,
    pub(crate) format: String,
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

        // verify format is supported
        Repo::is_supported(&config.format)?;

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
    #[serde(skip)]
    externals: HashMap<String, Repo>,
}

impl Config {
    pub(super) fn new(
        config_dir: &Utf8Path,
        db_dir: &Utf8Path,
        create: bool,
    ) -> crate::Result<Config> {
        let config_dir = config_dir.join("repos");
        let repo_dir = db_dir.join("repos");

        // create paths on request
        if create {
            for path in [&config_dir, &repo_dir] {
                fs::create_dir_all(path).map_err(|e| Error::Config(e.to_string()))?;
            }
        }

        let mut configs = Vec::<(String, RepoConfig)>::new();
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

        // create hash tables of repos ordered by priority
        let mut repos = IndexMap::<String, Repo>::new();
        for (name, c) in configs.into_iter() {
            // ignore unsynced or nonexistent repos
            match Repo::from_format(&name, c.priority, &c.location, &c.format) {
                Ok(repo) => {
                    repos.insert(name, repo);
                }
                Err(err) => warn!("{err}"),
            }
        }

        // sort repos by priority then by name
        repos.sort_by(|_k1, v1, _k2, v2| v1.cmp(v2));

        Ok(Config {
            config_dir,
            repo_dir,
            repos,
            ..Default::default()
        })
    }

    pub(super) fn finalize(&self) -> crate::Result<()> {
        for repo in self.repos.values() {
            repo.finalize()?;
        }
        Ok(())
    }

    /// Add local repo from a filesystem path.
    pub(super) fn add_path(
        &mut self,
        name: &str,
        priority: i32,
        path: &str,
    ) -> crate::Result<Repo> {
        if self.repos.get(name).is_some() {
            return Err(Error::Config(format!("existing repo: {name}")));
        }

        let path = Utf8PathBuf::from(path);
        if !path.exists() {
            return Err(Error::Config(format!("nonexistent repo path: {path:?}")));
        }

        Repo::from_path(name, priority, path)
    }

    /// Add external repo from a URI.
    pub(super) fn add_uri(&mut self, name: &str, priority: i32, uri: &str) -> crate::Result<Repo> {
        if self.repos.get(name).is_some() {
            return Err(Error::Config(format!("existing repo: {name}")));
        }

        let config = RepoConfig {
            location: self.repo_dir.join(name),
            priority,
            sync: Some(Syncer::from_str(uri)?),
            ..Default::default()
        };
        config.sync()?;

        let repo = Repo::from_path(name, priority, config.location)?;

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
        match self.repos.get(name) {
            Some(_) => Err(Error::Config(format!("existing repo: {name}"))),
            None => {
                let path = self.repo_dir.join(name);
                // create temporary repo and persist it to disk
                let temp_repo = TempRepo::new(name, Some(&self.repo_dir), None)?;
                temp_repo.persist(Some(&path))?;
                // add repo to config
                self.add_path(name, priority, path.as_str())
            }
        }
    }

    #[cfg(test)]
    pub(super) fn create_temp(
        &mut self,
        name: &str,
        priority: i32,
    ) -> crate::Result<(TempRepo, Repo)> {
        match self.repos.get(name) {
            Some(_) => Err(Error::Config(format!("existing repo: {name}"))),
            None => {
                let temp_repo = TempRepo::new(name, None, None)?;
                let r = self.add_path(name, priority, temp_repo.path.as_str())?;
                Ok((temp_repo, r))
            }
        }
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
                    let path = self.config_dir.join(&name);
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

        let mut failed = Vec::<(&str, Error)>::new();
        for name in repos {
            if let Some(repo) = self.repos.get(name) {
                if let Err(e) = repo.sync() {
                    failed.push((name, e));
                }
            }
        }

        match failed.is_empty() {
            true => Ok(()),
            false => {
                let errors = failed
                    .iter()
                    .map(|(name, e)| format!("{name}: {e}"))
                    .collect::<Vec<String>>()
                    .join("\n\t");
                Err(Error::Config(format!("failed syncing:\n\t{errors}")))
            }
        }
    }

    pub fn iter(&self) -> ReposIter {
        self.into_iter()
    }

    /// Get a configured repo, falling back to external repos on nonexistence.
    pub fn get(&self, key: &str) -> Option<&Repo> {
        self.repos.get(key).or_else(|| self.externals.get(key))
    }

    pub(super) fn insert(&mut self, id: &str, repo: Repo, external: bool) {
        // populate external repo mapping for masters finalization
        if external {
            if let Some(r) = repo.as_ebuild() {
                self.externals.insert(r.path().to_string(), repo.clone());
            }
        }

        self.repos.insert(id.to_string(), repo);
        self.sort()
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
        ReposIter {
            iter: self.repos.iter(),
        }
    }
}

impl<'a> Iterator for ReposIter<'a> {
    type Item = (&'a str, &'a Repo);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(id, repo)| (id.as_str(), repo))
    }
}
