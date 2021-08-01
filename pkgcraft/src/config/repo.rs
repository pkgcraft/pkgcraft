use std::cmp::Ordering;
use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::Error::ConfigError;
use crate::error::Result;
use crate::repo::Repository;

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
struct RepoConfig {
    location: String,
    format: String,
    priority: i32,
    url: Option<String>,
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

        let location = Path::new(&repo_conf.location);
        if !location.exists() {
            return Err(ConfigError(format!(
                "invalid repo config {:?}: nonexistent location: {:?}",
                &path, &location
            )));
        }

        // TODO: verify format is supported

        Ok(repo_conf)
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
    #[serde(default)] // https://github.com/mehcode/config-rs/issues/114
    configs: IndexMap<String, RepoConfig>,
    #[serde(default)] // https://github.com/mehcode/config-rs/issues/114
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
            let repo = Repository::from_format(&name, &config.location, &config.format)?;
            repos.insert(name.clone(), repo);
            configs.insert(name.clone(), config);
        }

        Ok(Config {
            config_dir,
            repo_dir,
            configs,
            repos,
        })
    }

    pub fn list(&self) -> Result<()> {
        for repo in self.repos.values() {
            println!("{}", repo);
        }
        Ok(())
    }

    // TODO: handling optional syncing
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

                // TODO: match against handled syncer URLs
                location = match uri.starts_with("https://") {
                    true => {
                        config.url = Some(uri.to_string());
                        location = self.repo_dir.join(&name);
                        if location.exists() {
                            return Err(ConfigError(format!("existing repo: {:?}", &location)));
                        }
                        // TODO: sync repo
                        location
                    }
                    false => {
                        location = match location.starts_with(&self.repo_dir) {
                            true => location,
                            false => {
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
                                location
                            }
                        };
                        location
                    }
                };

                let location = location
                    .to_str()
                    .ok_or_else(|| ConfigError(format!("bad repo location: {:?}", &location)))?
                    .to_string();

                // TODO: determine format from repo
                let (format, repo) = Repository::from_path(&name, &location)?;
                config.format = format;
                config.location = location;

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

    pub fn del(&mut self, repos: &[&str], _clean: bool) -> Result<()> {
        let mut failed: Vec<&str> = Vec::new();
        for repo in repos {
            match self.repos.remove(repo as &str) {
                Some(_) => (),
                None => failed.push(repo),
            }
        }

        match failed.is_empty() {
            true => Ok(()),
            false => Err(ConfigError(format!(
                "failed removing: {}",
                failed.join(", ")
            ))),
        }
    }

    // TODO: add syncing support
    pub fn sync(&mut self, repos: &[&str]) -> Result<()> {
        let mut failed: Vec<&str> = Vec::new();
        for repo in repos {
            match self.repos.get(repo as &str) {
                Some(_) => (),
                None => failed.push(repo),
            }
        }

        match failed.is_empty() {
            true => Ok(()),
            false => Err(ConfigError(format!(
                "failed syncing: {}",
                failed.join(", ")
            ))),
        }
    }
}
