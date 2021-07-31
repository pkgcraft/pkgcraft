use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::repo::Repository;

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
struct RepoConfig {
    location: String,
    format: String,
    priority: i32,
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
    path: PathBuf,
    #[serde(default)] // https://github.com/mehcode/config-rs/issues/114
    configs: IndexMap<String, RepoConfig>,
    #[serde(default)] // https://github.com/mehcode/config-rs/issues/114
    repos: IndexMap<String, Repository>,
}

impl Config {
    pub fn new(config_dir: &Path) -> Result<Config> {
        let path = config_dir.join("repos");

        // if no repo config dir exists, return the default
        if !path.exists() {
            return Ok(Config::default());
        }

        let mut repo_configs: Vec<(RepoConfig, String)> = Vec::new();
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            let p = entry.path();

            if p.is_file() {
                if let Some(name) = p
                    .file_name()
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
                    .filter(|s| !s.starts_with('.'))
                {
                    let data = fs::read_to_string(&p).map_err(|e| {
                        Error::ConfigError(format!("failed loading repo config {:?}: {}", &p, e))
                    })?;

                    let repo_conf: RepoConfig = toml::from_str(&data).map_err(|e| {
                        Error::ConfigError(format!(
                            "failed loading repo config toml {:?}: {}",
                            &p, e
                        ))
                    })?;
                    repo_configs.push((repo_conf, name));
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
            path,
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
    pub fn add(&mut self, name: &str, _uri: &str, _sync: bool) -> Result<()> {
        match self.repos.get(name) {
            Some(_) => Err(Error::ConfigError(format!("existing repo: {:?}", name))),
            None => {
                //let repo = T::from_path("bar")?;
                //self.repos.insert(name.to_string(), repo);
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
            false => Err(Error::ConfigError(format!(
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
            false => Err(Error::ConfigError(format!(
                "failed syncing: {}",
                failed.join(", ")
            ))),
        }
    }
}
