use std::cmp::Ordering;
use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::Error::ConfigError;
use crate::error::{Error, Result};
use crate::repo::Repository;
use crate::sync::Syncer;

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct RepoConfig {
    pub location: String,
    pub format: String,
    pub priority: i32,
    sync: Option<Syncer>,
}

impl RepoConfig {
    fn new(path: &Path) -> Result<Self> {
        let data = fs::read_to_string(&path)
            .map_err(|e| ConfigError(format!("failed loading repo config {:?}: {}", &path, e)))?;

        let repo_conf: RepoConfig = toml::from_str(&data).map_err(|e| {
            ConfigError(format!(
                "failed loading repo config toml {:?}: {}",
                &path, e
            ))
        })?;

        // verify format is supported
        Repository::supported(&repo_conf.format)?;

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
        self.priority.partial_cmp(&other.priority)
    }
}

impl Ord for RepoConfig {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    config_dir: PathBuf,
    repo_dir: PathBuf,
    #[serde(default)]
    pub configs: IndexMap<String, RepoConfig>,
    #[serde(default)]
    repos: IndexMap<String, Repository>,
}

impl Config {
    pub fn new(config_dir: &Path, db_dir: &Path) -> Result<Config> {
        let config_dir = config_dir.join("repos");
        let repo_dir = db_dir.join("repos");

        // if no repo config dir exists, return the default
        if !config_dir.exists() {
            return Ok(Config::default());
        }

        let mut repo_configs: Vec<(RepoConfig, String)> = Vec::new();
        for entry in fs::read_dir(&config_dir)? {
            let entry = entry?;
            let p = entry.path();

            if p.is_file() {
                if let Some(name) = p
                    .file_name()
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
                    .filter(|s| !s.starts_with('.'))
                {
                    // ignore bad configs
                    match RepoConfig::new(&p) {
                        Ok(repo_conf) => repo_configs.push((repo_conf, name)),
                        Err(err) => log::warn!("{}", err),
                    }
                }
            }
        }

        // sort repo configs by priority then by name
        repo_configs.sort();

        // create hash tables of repos ordered by priority
        let mut configs: IndexMap<String, RepoConfig> = Default::default();
        let mut repos: IndexMap<String, Repository> = Default::default();
        for (config, name) in repo_configs {
            // ignore unsynced or nonexistent repos
            match Repository::from_format(&name, &config.location, &config.format) {
                Ok(repo) => {
                    repos.insert(name.clone(), repo);
                }
                Err(err) => log::warn!("{}", err),
            }
            configs.insert(name.clone(), config);
        }

        Ok(Config {
            config_dir,
            repo_dir,
            configs,
            repos,
        })
    }

    pub fn add(&mut self, name: &str, uri: &str) -> Result<()> {
        let name = name.to_string();

        match self.configs.get(&name) {
            Some(c) => Err(ConfigError(format!(
                "existing repo: {:?} @ {:?}",
                &name, &c.location
            ))),
            None => {
                let mut config: RepoConfig = Default::default();
                let mut location = PathBuf::from(uri);

                let path_to_location = |p: PathBuf| -> Result<String> {
                    p.to_str()
                        .ok_or_else(|| ConfigError(format!("bad repo location: {:?}", &p)))
                        .map(|s| s.to_string())
                };

                match Syncer::from_str(uri) {
                    Ok(Syncer::Noop) | Err(_) => {
                        if !location.starts_with(&self.repo_dir) {
                            location = self.repo_dir.join(&name);
                            fs::create_dir_all(&self.repo_dir).map_err(|e| {
                                ConfigError(format!(
                                    "failed creating repo dir {:?}: {}",
                                    &self.repo_dir, &e
                                ))
                            })?;
                            symlink(&uri, &location).map_err(|e| {
                                ConfigError(format!(
                                    "failed symlinking repo {:?} to {:?}: {}",
                                    &uri, &location, &e
                                ))
                            })?;
                        }
                        config.location = path_to_location(location)?;
                    }
                    Ok(syncer) => {
                        location = self.repo_dir.join(&name);
                        if location.exists() {
                            return Err(ConfigError(format!("existing repo: {:?}", &location)));
                        }
                        config.sync = Some(syncer);
                        config.location = path_to_location(location)?;
                        config.sync()?;
                    }
                };

                let (format, repo) = Repository::from_path(&name, &config.location)?;
                config.format = format;

                // write repo config file to disk
                let repo_conf_data = toml::to_string(&config).map_err(|e| {
                    ConfigError(format!("failed serializing repo config to toml: {}", &e))
                })?;
                let path = self.config_dir.join(&name);
                let mut file = fs::File::create(&path).map_err(|e| {
                    ConfigError(format!(
                        "failed creating repo config file: {:?}: {}",
                        &path, &e
                    ))
                })?;
                file.write_all(repo_conf_data.as_bytes()).map_err(|e| {
                    ConfigError(format!(
                        "failed writing repo config file: {:?}: {}",
                        &path, &e
                    ))
                })?;

                self.repos.insert(name.clone(), repo);
                self.configs.insert(name.clone(), config);
                Ok(())
            }
        }
    }

    pub fn del(&mut self, repos: &[&str], clean: bool) -> Result<()> {
        for name in repos {
            // error out if repo config is missing
            let repo_config = self.config_from_id(name)?;
            // physical repo files are allowed to be missing
            if let Ok(_repo) = self.repo_from_id(name) {
                if clean {
                    fs::remove_dir_all(&repo_config.location).map_err(|e| {
                        ConfigError(format!(
                            "failed removing repo files: {:?}: {}",
                            &repo_config.location, &e
                        ))
                    })?;
                }
                self.repos.remove(name as &str);
            }

            if clean {
                let path = self.config_dir.join(&name);
                fs::remove_file(&path).map_err(|e| {
                    ConfigError(format!("failed removing repo config: {:?}: {}", &path, &e))
                })?;
            }
            self.configs.remove(name as &str);
        }
        Ok(())
    }

    fn repo_from_id<S: AsRef<str>>(&self, id: S) -> Result<&Repository> {
        let id = id.as_ref();
        match self.repos.get(id) {
            Some(repo) => Ok(repo),
            None => Err(ConfigError(format!("nonexistent repo: {:?}", id))),
        }
    }

    fn config_from_id<S: AsRef<str>>(&self, id: S) -> Result<&RepoConfig> {
        let id = id.as_ref();
        match self.configs.get(id) {
            Some(config) => Ok(config),
            None => Err(ConfigError(format!("nonexistent repo: {:?}", id))),
        }
    }

    // TODO: add concurrent syncing support with output progress
    pub fn sync(&mut self, repos: Option<Vec<&str>>) -> Result<()> {
        let repos = match repos {
            Some(names) => names,
            // sync all configured repos if none were passed
            None => self.configs.keys().map(|s| s.as_str()).collect(),
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
                    .map(|(name, e)| format!("{}: {}", name, e))
                    .collect::<Vec<String>>()
                    .join("\n\t");
                Err(ConfigError(format!("failed syncing:\n\t{}", errors)))
            }
        }
    }
}
