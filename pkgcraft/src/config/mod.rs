use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;

mod repo;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    cache_dir: PathBuf,
    config_dir: PathBuf,
    pub repos: repo::Config,
}

impl Default for Config {
    fn default() -> Self {
        Config::new("pkgcraft", "", false).unwrap()
    }
}

impl Config {
    pub fn new(name: &str, prefix: &str, create: bool) -> Result<Config> {
        let home = env::var("HOME")?;

        // pull user cache dir from $XDG_CACHE_HOME, otherwise $HOME/.cache
        let mut user_cache: PathBuf = match env::var("XDG_CACHE_HOME") {
            Ok(x) => PathBuf::from(x),
            Err(_) => [&home, ".cache"].iter().collect(),
        };
        user_cache.push(name);

        // pull user cache dir from $XDG_CONFIG_HOME, otherwise $HOME/.config
        let mut user_config: PathBuf = match env::var("XDG_CONFIG_HOME") {
            Ok(x) => PathBuf::from(x),
            Err(_) => [&home, ".config"].iter().collect(),
        };
        user_config.push(name);

        let mut system_cache = PathBuf::from("/var/cache");
        let mut system_config = PathBuf::from(format!("/etc/{}", name));

        // append non-empty prefix
        if !prefix.is_empty() {
            let prefix = PathBuf::from(prefix);
            let prefixed = |path: &PathBuf| -> PathBuf {
                prefix.join(path.strip_prefix("/").unwrap_or(&path))
            };
            user_cache = prefixed(&user_cache);
            user_config = prefixed(&user_config);
            system_cache = prefixed(&system_cache);
            system_config = prefixed(&system_config);
        }

        // Config precedence:
        //  - existing user config
        //  - existing system config
        //  - create new user config
        let (cache_dir, config_dir) = match (user_config.exists(), system_config.exists()) {
            (true, _) => (user_cache, user_config),
            (_, true) => (system_cache, system_config),
            _ => (user_cache, user_config),
        };

        // create dirs on request
        if create {
            fs::create_dir_all(&cache_dir)?;
            fs::create_dir_all(&config_dir)?;
        }

        let repos = repo::Config::new(&config_dir)?;

        Ok(Config {
            cache_dir,
            config_dir,
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

        // prefixed
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

        // prefixed
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
        assert!(Config::new("pkgcraft", "", false).is_err());
    }
}
