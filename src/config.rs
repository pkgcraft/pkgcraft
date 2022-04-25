use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::error::Error;

mod repo;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ConfigPath {
    pub cache: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub db: PathBuf,
    pub run: PathBuf,
}

impl ConfigPath {
    fn new(name: &str, prefix: &str, create: bool) -> crate::Result<ConfigPath> {
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
            Ok(x) => prefixed([&x, name].iter().collect::<PathBuf>()),
            Err(_) => prefixed([&home, ".config", name].iter().collect()),
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
                    Ok(x) => prefixed([&x, name].iter().collect::<PathBuf>()),
                    Err(_) => prefixed([&home, ".cache", name].iter().collect::<PathBuf>()),
                };

                // pull user data path from $XDG_DATA_HOME, otherwise $HOME/.local/share
                data = match env::var("XDG_DATA_HOME") {
                    Ok(x) => prefixed([&x, name].iter().collect::<PathBuf>()),
                    Err(_) => {
                        prefixed([&home, ".local", "share", name].iter().collect::<PathBuf>())
                    }
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

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub path: ConfigPath,
    pub repos: repo::Config,
}

thread_local! {
    static CURRENT_CONFIG: RwLock<Arc<Config>> = RwLock::new(Default::default());
}

impl Config {
    pub fn new(name: &str, prefix: &str, create: bool) -> crate::Result<Arc<Config>> {
        let path = ConfigPath::new(name, prefix, create)?;
        let repos = repo::Config::new(&path.config, &path.db, create)?;
        let config = Config { path, repos };
        config.make_current();
        Ok(Config::current())
    }

    pub(crate) fn current() -> Arc<Config> {
        CURRENT_CONFIG.with(|c| c.read().unwrap().clone())
    }

    pub(crate) fn make_current(self) {
        CURRENT_CONFIG.with(|c| *c.write().unwrap() = Arc::new(self))
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
