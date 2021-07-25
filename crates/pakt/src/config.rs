use std::env;
use std::fs;
use std::path::PathBuf;

use pkgcraft::error::Result;

#[derive(Debug, PartialEq)]
pub struct Config {
    cache: PathBuf,
    config: PathBuf,
}

impl Config {
    pub fn new(prefix: &str, create: bool) -> Result<Config> {
        let mut user_cache = user_dir("XDG_CACHE_HOME", ".cache")?;
        let mut user_config = user_dir("XDG_CONFIG_HOME", ".config")?;
        let mut system_cache = PathBuf::from("/var/cache");
        let mut system_config = PathBuf::from("/etc/pakt");

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
        let (cache, config) = match (user_config.exists(), system_config.exists()) {
            (true, _) => (user_cache, user_config),
            (_, true) => (system_cache, system_config),
            _ => (user_cache, user_config),
        };

        // create dirs on request
        if create {
            fs::create_dir_all(&cache)?;
            fs::create_dir_all(&config)?;
        }

        Ok(Config { cache, config })
    }
}

fn user_dir(xdg_var: &str, fallback: &str) -> Result<PathBuf> {
    let mut path = PathBuf::new();

    match env::var(xdg_var) {
        Ok(s) => {
            path.push(s);
        }
        Err(_) => {
            path.push(env::var("HOME")?);
            path.push(fallback);
        }
    };

    path.push("pakt");
    Ok(path)
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
        let config = Config::new("", false).unwrap();
        assert_eq!(config.cache, PathBuf::from("/cache/pakt"));
        assert_eq!(config.config, PathBuf::from("/config/pakt"));

        // prefixed
        let config = Config::new("/prefix", false).unwrap();
        assert_eq!(config.cache, PathBuf::from("/prefix/cache/pakt"));
        assert_eq!(config.config, PathBuf::from("/prefix/config/pakt"));

        env::remove_var("XDG_CACHE_HOME");
        env::remove_var("XDG_CONFIG_HOME");

        // XDG var is unset and HOME is set
        let config = Config::new("", false).unwrap();
        assert_eq!(config.cache, PathBuf::from("/home/user/.cache/pakt"));
        assert_eq!(config.config, PathBuf::from("/home/user/.config/pakt"));

        // prefixed
        let config = Config::new("/prefix", false).unwrap();
        assert_eq!(config.cache, PathBuf::from("/prefix/home/user/.cache/pakt"));
        assert_eq!(
            config.config,
            PathBuf::from("/prefix/home/user/.config/pakt")
        );
        env::remove_var("HOME");

        // XDG var and HOME are unset
        assert!(Config::new("", false).is_err());
    }
}
