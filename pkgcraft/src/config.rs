use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Error;

mod repo;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    cache_dir: PathBuf,
    config_dir: PathBuf,
    data_dir: PathBuf,
    db_dir: PathBuf,
    pub repos: repo::Config,
}

impl Config {
    pub fn new(name: &str, prefix: &str, create: bool) -> crate::Result<Config> {
        let home = env::var("HOME").ok().unwrap_or_else(|| "/root".to_string());
        let (config_dir, cache_dir, data_dir, db_dir): (PathBuf, PathBuf, PathBuf, PathBuf);

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

        let system_config = prefixed(PathBuf::from(format!("/etc/{}", name)));

        // determine if user config or system config will be used
        config_dir = match (
            user_config.exists(),
            system_config.exists() || home == "/root",
        ) {
            (false, true) => {
                cache_dir = prefixed(PathBuf::from(format!("/var/cache/{}", name)));
                data_dir = prefixed(PathBuf::from(format!("/usr/share/{}", name)));
                db_dir = prefixed(PathBuf::from(format!("/var/db/{}", name)));
                system_config
            }
            _ => {
                // pull user cache dir from $XDG_CACHE_HOME, otherwise $HOME/.cache
                cache_dir = match env::var("XDG_CACHE_HOME") {
                    Ok(x) => prefixed([&x, name].iter().collect::<PathBuf>()),
                    Err(_) => prefixed([&home, ".cache", name].iter().collect::<PathBuf>()),
                };

                // pull user data dir from $XDG_DATA_HOME, otherwise $HOME/.local/share
                data_dir = match env::var("XDG_DATA_HOME") {
                    Ok(x) => prefixed([&x, name].iter().collect::<PathBuf>()),
                    Err(_) => {
                        prefixed([&home, ".local", "share", name].iter().collect::<PathBuf>())
                    }
                };

                db_dir = data_dir.clone();
                user_config
            }
        };

        // create dirs on request
        if create {
            fs::create_dir_all(&cache_dir).map_err(|e| Error::Config(e.to_string()))?;
            fs::create_dir_all(&config_dir).map_err(|e| Error::Config(e.to_string()))?;
            fs::create_dir_all(&data_dir).map_err(|e| Error::Config(e.to_string()))?;
            fs::create_dir_all(&db_dir).map_err(|e| Error::Config(e.to_string()))?;
        }

        let repos = repo::Config::new(&config_dir, &db_dir)?;

        Ok(Config {
            cache_dir,
            config_dir,
            data_dir,
            db_dir,
            repos,
        })
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
        assert_eq!(config.cache_dir, PathBuf::from("/cache/pkgcraft"));
        assert_eq!(config.config_dir, PathBuf::from("/config/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix", false).unwrap();
        assert_eq!(config.cache_dir, PathBuf::from("/prefix/cache/pkgcraft"));
        assert_eq!(config.config_dir, PathBuf::from("/prefix/config/pkgcraft"));

        env::remove_var("XDG_CACHE_HOME");
        env::remove_var("XDG_CONFIG_HOME");

        // XDG var is unset and HOME is set
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(
            config.cache_dir,
            PathBuf::from("/home/user/.cache/pkgcraft")
        );
        assert_eq!(
            config.config_dir,
            PathBuf::from("/home/user/.config/pkgcraft")
        );

        // prefix
        let config = Config::new("pkgcraft", "/prefix", false).unwrap();
        assert_eq!(
            config.cache_dir,
            PathBuf::from("/prefix/home/user/.cache/pkgcraft")
        );
        assert_eq!(
            config.config_dir,
            PathBuf::from("/prefix/home/user/.config/pkgcraft")
        );
        env::remove_var("HOME");

        // XDG var and HOME are unset
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(config.config_dir, PathBuf::from("/etc/pkgcraft"));
    }
}
