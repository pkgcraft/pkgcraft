use std::cmp::Ordering;
use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::repo::ebuild::TempRepo;
use crate::repo::Repository;
use crate::sync::Syncer;

#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct RepoConfig {
    pub location: PathBuf,
    pub format: String,
    pub priority: i32,
    sync: Option<Syncer>,
}

impl RepoConfig {
    fn new(path: &Path) -> Result<Self, Error> {
        let data = fs::read_to_string(&path)
            .map_err(|e| Error::Config(format!("failed loading repo config {:?}: {}", &path, e)))?;

        let repo_conf: RepoConfig = toml::from_str(&data).map_err(|e| {
            Error::Config(format!(
                "failed loading repo config toml {:?}: {}",
                &path, e
            ))
        })?;

        // verify format is supported
        Repository::is_supported(&repo_conf.format)?;

        Ok(repo_conf)
    }

    fn sync(&self) -> Result<(), Error> {
        match &self.sync {
            Some(syncer) => syncer.sync(&self.location),
            None => Ok(()),
        }
    }
}

impl PartialOrd for RepoConfig {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => self.location.partial_cmp(&other.location),
            cmp => Some(cmp),
        }
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
    #[serde(skip)]
    pub configs: IndexMap<String, RepoConfig>,
    #[serde(skip)]
    repos: IndexMap<String, Repository>,
}

impl Config {
    pub fn new(config_dir: &Path, db_dir: &Path) -> Result<Config, Error> {
        let config_dir = config_dir.join("repos");
        let repo_dir = db_dir.join("repos");

        // if no repo config dir exists, return the default
        if !config_dir.exists() {
            return Ok(Config::default());
        }

        let mut repo_configs: Vec<(RepoConfig, String)> = Vec::new();
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

    pub fn add(&mut self, name: &str, uri: &str) -> Result<(), Error> {
        let name = name.to_string();

        match self.configs.get(&name) {
            Some(c) => Err(Error::Config(format!(
                "existing repo: {:?} @ {:?}",
                &name, &c.location
            ))),
            None => {
                let dest_dir = self.repo_dir.join(&name);
                if dest_dir.exists() {
                    return Err(Error::Config(format!("existing repo: {:?}", &dest_dir)));
                }

                let mut config: RepoConfig = RepoConfig {
                    location: dest_dir.clone(),
                    ..Default::default()
                };

                fs::create_dir_all(&self.repo_dir).map_err(|e| {
                    Error::Config(format!(
                        "failed creating repo dir {:?}: {}",
                        &self.repo_dir, &e
                    ))
                })?;

                match Syncer::from_str(uri) {
                    Ok(Syncer::Noop) | Err(_) => {
                        let mut path = PathBuf::from(uri);
                        if path.is_relative() {
                            path = fs::canonicalize(&path).map_err(|e| {
                                Error::Config(format!(
                                    "failed canonicalizing path {:?}: {}",
                                    &path, &e
                                ))
                            })?;
                        }
                        if path.exists() {
                            symlink(&path, &dest_dir).map_err(|e| {
                                Error::Config(format!(
                                    "failed symlinking repo {:?} to {:?}: {}",
                                    &path, &dest_dir, &e
                                ))
                            })?;
                        } else {
                            return Err(Error::Config(format!(
                                "nonexistent repo path: {:?}",
                                &path
                            )));
                        }
                    }
                    Ok(syncer) => {
                        config.sync = Some(syncer);
                        config.sync()?;
                    }
                };

                let (format, repo) = Repository::from_path(&name, &config.location)?;
                config.format = format.to_string();

                // write repo config file to disk
                let repo_conf_data = toml::to_string(&config).map_err(|e| {
                    Error::Config(format!("failed serializing repo config to toml: {}", &e))
                })?;
                let path = self.config_dir.join(&name);
                let mut file = fs::File::create(&path).map_err(|e| {
                    Error::Config(format!(
                        "failed creating repo config file: {:?}: {}",
                        &path, &e
                    ))
                })?;
                file.write_all(repo_conf_data.as_bytes()).map_err(|e| {
                    Error::Config(format!(
                        "failed writing repo config file: {:?}: {}",
                        &path, &e
                    ))
                })?;

                // TODO: re-sort config/repo maps by priority, then name
                self.configs.insert(name.clone(), config);
                self.repos.insert(name.clone(), repo);
                Ok(())
            }
        }
    }

    pub fn create(&mut self, name: &str) -> Result<(), Error> {
        match self.configs.get(name) {
            Some(c) => Err(Error::Config(format!(
                "existing repo: {:?} @ {:?}",
                name, c.location
            ))),
            None => {
                // create temporary repo and persist it to disk
                let temp_repo_path = TempRepo::new(name, Some(&self.repo_dir), None)?.persist();
                // rename new repo dir what it should be called
                let repo_path = self.repo_dir.join(name);
                fs::rename(&temp_repo_path, &repo_path).map_err(|e| {
                    Error::Config(format!(
                        "failed renaming repo: {:?} -> {:?}: {}",
                        &temp_repo_path, &repo_path, e
                    ))
                })?;
                // add repo to config
                let location = repo_path
                    .to_str()
                    .ok_or_else(|| Error::Config(format!("invalid repo name: {:?}", name)))?;
                self.add(name, location)
            }
        }
    }

    pub fn del(&mut self, repos: &[&str], clean: bool) -> Result<(), Error> {
        for name in repos {
            // error out if repo config is missing
            let repo_config = self.config_from_id(name)?;
            // physical repo files are allowed to be missing
            if let Ok(_repo) = self.repo_from_id(name) {
                if clean {
                    fs::remove_dir_all(&repo_config.location).map_err(|e| {
                        Error::Config(format!(
                            "failed removing repo files: {:?}: {}",
                            &repo_config.location, &e
                        ))
                    })?;
                }
                self.repos.shift_remove(name as &str);
            }

            if clean {
                let path = self.config_dir.join(&name);
                fs::remove_file(&path).map_err(|e| {
                    Error::Config(format!("failed removing repo config: {:?}: {}", &path, &e))
                })?;
            }
            self.configs.shift_remove(name as &str);
        }
        Ok(())
    }

    fn repo_from_id<S: AsRef<str>>(&self, id: S) -> Result<&Repository, Error> {
        let id = id.as_ref();
        match self.repos.get(id) {
            Some(repo) => Ok(repo),
            None => Err(Error::Config(format!("nonexistent repo: {:?}", id))),
        }
    }

    fn config_from_id<S: AsRef<str>>(&self, id: S) -> Result<&RepoConfig, Error> {
        let id = id.as_ref();
        match self.configs.get(id) {
            Some(config) => Ok(config),
            None => Err(Error::Config(format!("nonexistent repo: {:?}", id))),
        }
    }

    // TODO: add concurrent syncing support with output progress
    pub fn sync(&mut self, repos: Option<Vec<&str>>) -> Result<(), Error> {
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
                Err(Error::Config(format!("failed syncing:\n\t{}", errors)))
            }
        }
    }
}
