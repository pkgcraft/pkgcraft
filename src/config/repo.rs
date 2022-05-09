use std::cmp::Ordering;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::repo::ebuild::TempRepo;
use crate::repo::Repository;
use crate::sync::Syncer;
use crate::{Error, Result};

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RepoConfig {
    pub location: PathBuf,
    pub format: String,
    pub priority: i32,
    sync: Option<Syncer>,
}

impl RepoConfig {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let data = fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("failed loading repo config {path:?}: {e}")))?;

        let repo_conf: RepoConfig = toml::from_str(&data)
            .map_err(|e| Error::Config(format!("failed loading repo config toml {path:?}: {e}")))?;

        // verify format is supported
        Repository::is_supported(&repo_conf.format)?;

        Ok(repo_conf)
    }

    fn sync(&self) -> Result<()> {
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
    config_dir: PathBuf,
    repo_dir: PathBuf,
    #[serde(skip)]
    pub configs: IndexMap<String, RepoConfig>,
    #[serde(skip)]
    pub repos: IndexMap<String, Arc<Repository>>,
}

impl Config {
    pub fn new<P: AsRef<Path>>(config_dir: P, db_dir: P, create: bool) -> Result<Config> {
        let (config_dir, db_dir) = (config_dir.as_ref(), db_dir.as_ref());
        let config_dir = config_dir.join("repos");
        let repo_dir = db_dir.join("repos");

        // create paths on request
        if create {
            for path in [&config_dir, &repo_dir] {
                fs::create_dir_all(path).map_err(|e| Error::Config(e.to_string()))?;
            }
        }

        let mut configs = IndexMap::<String, RepoConfig>::new();
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
                            Ok(repo_conf) => {
                                configs.insert(name, repo_conf);
                            }
                            Err(err) => warn!("{err}"),
                        }
                    }
                }
            }

            // sort configs by priority then by name
            configs.sort_by(|_k1, v1, _k2, v2| v1.cmp(v2));
        }

        // create hash tables of repos ordered by priority
        let mut repos = IndexMap::<String, Arc<Repository>>::new();
        for (name, config) in configs.iter() {
            // ignore unsynced or nonexistent repos
            match Repository::from_format(&name, &config.location, &config.format) {
                Ok(repo) => {
                    repos.insert(name.clone(), Arc::new(repo));
                }
                Err(err) => warn!("{err}"),
            }
        }

        Ok(Config {
            config_dir,
            repo_dir,
            configs,
            repos,
        })
    }

    pub fn add(&mut self, name: &str, uri: &str) -> Result<()> {
        let dest_dir = self.repo_dir.join(name);

        if let Some(c) = self.configs.get(name) {
            return Err(Error::Config(format!("existing repo: {name:?} @ {:?}", &c.location)));
        }

        let mut config: RepoConfig = Default::default();
        let path = Path::new(uri);

        let repo = match path.exists() {
            true => {
                // add local, external repo
                let path = path.canonicalize().map_err(|e| {
                    Error::Config(format!("failed canonicalizing repo path {path:?}: {e}"))
                })?;
                let (format, repo) = Repository::from_path(name, path)?;
                config.format = format.to_string();
                repo
            }
            false => {
                config.location = dest_dir;
                config.sync = Some(Syncer::from_str(uri)?);
                config.sync()?;

                let (format, repo) = Repository::from_path(name, &config.location)?;
                config.format = format.to_string();

                // write repo config file to disk
                let repo_conf_data = toml::to_string(&config).map_err(|e| {
                    Error::Config(format!("failed serializing repo config to toml: {e}"))
                })?;
                let path = self.config_dir.join(name);
                let mut file = fs::File::create(&path).map_err(|e| {
                    Error::Config(format!("failed creating repo config file: {path:?}: {e}"))
                })?;
                file.write_all(repo_conf_data.as_bytes()).map_err(|e| {
                    Error::Config(format!("failed writing repo config file: {path:?}: {e}"))
                })?;

                repo
            }
        };

        let (configs, repos) = (&mut self.configs, &mut self.repos);
        configs.insert(name.to_string(), config);
        // re-sort configs by RepoConfig ordering
        configs.sort_by(|_k1, v1, _k2, v2| v1.cmp(v2));
        repos.insert(name.to_string(), Arc::new(repo));
        // use sorted configs to re-sort repos
        repos.sort_by(|k1, _v1, k2, _v2| {
            let k1_index = configs.get_index_of(k1).unwrap();
            let k2_index = configs.get_index_of(k2).unwrap();
            k1_index.cmp(&k2_index)
        });
        Ok(())
    }

    pub fn create(&mut self, name: &str) -> Result<()> {
        match self.configs.get(name) {
            Some(c) => Err(Error::Config(format!("existing repo: {name:?} @ {:?}", c.location))),
            None => {
                let repo_path = self.repo_dir.join(name);
                let location = repo_path
                    .to_str()
                    .ok_or_else(|| Error::Config(format!("invalid repo name: {name:?}")))?;
                // create temporary repo and persist it to disk
                let temp_repo = TempRepo::new(name, Some(&self.repo_dir), None)?;
                temp_repo.persist(Some(&repo_path))?;
                // add repo to config
                self.add(name, location)
            }
        }
    }

    pub fn del<S: AsRef<str>>(&mut self, repos: &[S], clean: bool) -> Result<()> {
        for name in repos {
            let name = name.as_ref();
            // error out if repo config is missing
            let repo_config = self.config_from_id(name)?;
            // physical repo files are allowed to be missing
            if let Ok(_repo) = self.repo_from_id(name) {
                if clean {
                    fs::remove_dir_all(&repo_config.location).map_err(|e| {
                        Error::Config(format!(
                            "failed removing repo files: {:?}: {e}",
                            &repo_config.location
                        ))
                    })?;
                }
                self.repos.shift_remove(name as &str);
            }

            if clean {
                let path = self.config_dir.join(&name);
                fs::remove_file(&path).map_err(|e| {
                    Error::Config(format!("failed removing repo config: {path:?}: {e}"))
                })?;
            }
            self.configs.shift_remove(name as &str);
        }
        Ok(())
    }

    fn repo_from_id<S: AsRef<str>>(&self, id: S) -> Result<&Repository> {
        let id = id.as_ref();
        match self.repos.get(id) {
            Some(repo) => Ok(repo),
            None => Err(Error::Config(format!("nonexistent repo: {id:?}"))),
        }
    }

    fn config_from_id<S: AsRef<str>>(&self, id: S) -> Result<&RepoConfig> {
        let id = id.as_ref();
        match self.configs.get(id) {
            Some(config) => Ok(config),
            None => Err(Error::Config(format!("nonexistent repo: {id:?}"))),
        }
    }

    // TODO: add concurrent syncing support with output progress
    pub fn sync<S: AsRef<str>>(&mut self, repos: Vec<S>) -> Result<()> {
        let repos: Vec<&str> = match &repos {
            names if !names.is_empty() => names.iter().map(|s| s.as_ref()).collect(),
            // sync all configured repos if none were passed
            _ => self.configs.keys().map(|s| s.as_str()).collect(),
        };

        let mut failed: Vec<(&str, Error)> = Vec::new();
        for name in repos {
            let repo_config = self.config_from_id(name)?;
            if let Err(e) = repo_config.sync() {
                failed.push((name, e));
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
}
