use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::macros::build_from_paths;
use crate::repo::Repo;
use crate::{Error, Result};
pub(crate) use repo::RepoConfig;

mod repo;

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct ConfigPath {
    pub cache: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub db: PathBuf,
    pub run: PathBuf,
}

impl ConfigPath {
    fn new(name: &str, prefix: &str, create: bool) -> Result<ConfigPath> {
        let home = env::var("HOME").ok().unwrap_or_else(|| "/root".to_string());
        let (config, cache, data, db, run): (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf);

        // prefix a given path
        let prefixed = |p: PathBuf| -> PathBuf {
            match prefix.is_empty() {
                true => p,
                false => PathBuf::from(prefix).join(p.strip_prefix("/").unwrap_or(&p)),
            }
        };

        // pull user config from $XDG_CONFIG_HOME, otherwise $HOME/.config
        let user_config: PathBuf = match env::var("XDG_CONFIG_HOME") {
            Ok(x) => prefixed(build_from_paths!(&x, name)),
            Err(_) => prefixed(build_from_paths!(&home, ".config", name)),
        };

        let system_config = prefixed(PathBuf::from(format!("/etc/{name}")));

        // determine if user config or system config will be used
        config = match (user_config.exists(), system_config.exists() || home == "/root") {
            (false, true) => {
                cache = prefixed(PathBuf::from(format!("/var/cache/{name}")));
                data = prefixed(PathBuf::from(format!("/usr/share/{name}")));
                db = prefixed(PathBuf::from(format!("/var/db/{name}")));
                run = prefixed(PathBuf::from(format!("/run/{name}")));
                system_config
            }
            _ => {
                // pull user cache path from $XDG_CACHE_HOME, otherwise $HOME/.cache
                cache = match env::var("XDG_CACHE_HOME") {
                    Ok(x) => prefixed(build_from_paths!(&x, name)),
                    Err(_) => prefixed(build_from_paths!(&home, ".cache", name)),
                };

                // pull user data path from $XDG_DATA_HOME, otherwise $HOME/.local/share
                data = match env::var("XDG_DATA_HOME") {
                    Ok(x) => prefixed(build_from_paths!(&x, name)),
                    Err(_) => prefixed(build_from_paths!(&home, ".local", "share", name)),
                };

                db = data.clone();
                run = cache.clone();
                user_config
            }
        };

        // create paths on request
        if create {
            for path in [&cache, &config, &data, &db, &run] {
                fs::create_dir_all(path).map_err(|e| Error::Config(e.to_string()))?;
            }
        }

        Ok(ConfigPath {
            cache,
            config,
            data,
            db,
            run,
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    pub path: ConfigPath,
    pub repos: repo::Config,
}

static CURRENT_CONFIG: Lazy<RwLock<Arc<Config>>> = Lazy::new(|| RwLock::new(Default::default()));

impl Config {
    pub fn new(name: &str, prefix: &str, create: bool) -> Result<Config> {
        let path = ConfigPath::new(name, prefix, create)?;
        let repos = repo::Config::new(&path.config, &path.db, create)?;
        let config = Config { path, repos };
        Config::make_current(config.clone())?;
        Ok(config)
    }

    pub fn current() -> Arc<Config> {
        CURRENT_CONFIG.read().unwrap().clone()
    }

    pub fn make_current(config: Config) -> Result<()> {
        *CURRENT_CONFIG.write().unwrap() = Arc::new(config);
        Config::current().repos.finalize()
    }

    pub fn add_repo(&mut self, name: &str, priority: i32, uri: &str) -> Result<Repo> {
        let r = self.repos.add(name, priority, uri)?;
        Config::make_current(self.clone())?;
        Ok(r)
    }

    pub fn create_repo(&mut self, name: &str, priority: i32) -> Result<Repo> {
        let r = self.repos.create(name, priority)?;
        Config::make_current(self.clone())?;
        Ok(r)
    }

    pub fn del_repos<S: AsRef<str>>(&mut self, repos: &[S], clean: bool) -> Result<()> {
        self.repos.del(repos, clean)?;
        Config::make_current(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn test_config() {
        env::set_var("XDG_CACHE_HOME", "/cache");
        env::set_var("XDG_CONFIG_HOME", "/config");
        env::set_var("HOME", "/home/user");

        // XDG var and HOME are set
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(config.path.cache, PathBuf::from("/cache/pkgcraft"));
        assert_eq!(config.path.config, PathBuf::from("/config/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix", false).unwrap();
        assert_eq!(config.path.cache, PathBuf::from("/prefix/cache/pkgcraft"));
        assert_eq!(config.path.config, PathBuf::from("/prefix/config/pkgcraft"));

        env::remove_var("XDG_CACHE_HOME");
        env::remove_var("XDG_CONFIG_HOME");

        // XDG var is unset and HOME is set
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(config.path.cache, PathBuf::from("/home/user/.cache/pkgcraft"));
        assert_eq!(config.path.config, PathBuf::from("/home/user/.config/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix", false).unwrap();
        assert_eq!(config.path.cache, PathBuf::from("/prefix/home/user/.cache/pkgcraft"));
        assert_eq!(config.path.config, PathBuf::from("/prefix/home/user/.config/pkgcraft"));
        env::remove_var("HOME");

        // XDG var and HOME are unset
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(config.path.config, PathBuf::from("/etc/pkgcraft"));
    }
}
