use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};

use crate::error::{Error, Result};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    path: PathBuf,
    #[serde(default)] // https://github.com/mehcode/config-rs/issues/114
    repos: HashSet<String>,
}

impl Config {
    pub fn new(config_dir: &Path) -> Result<Config> {
        let path = config_dir.join("repos");
        let repos: HashSet<String>;

        if let Ok(repo_paths) = fs::read_dir(&path) {
            repos = repo_paths
                .filter_map(|entry| {
                    entry.ok().and_then(|e| {
                        e.path()
                            .file_name()
                            .and_then(|p| p.to_str().map(|s| s.to_string()))
                    })
                })
                .collect();
        } else {
            repos = HashSet::new();
        }

        Ok(Config { path, repos })
    }

    pub fn add<'a>(&mut self, name: &'a str, uri: &'a str) -> Result<&'a str> {
        match self.repos.get(name) {
            Some(_) => Err(Error::ConfigError(format!("existing repo: {:?}", name))),
            None => {
                self.repos.insert(name.to_string());
                Ok(name)
            }
        }
    }

    pub fn del<'a>(&mut self, name: &'a str) -> Result<&'a str> {
        match self.repos.remove(name) {
            true => Ok(name),
            false => Err(Error::ConfigError(format!("nonexistent repo: {:?}", name))),
        }
    }
}
