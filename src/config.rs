use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use camino::{Utf8Path, Utf8PathBuf};
use ini::Ini;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::macros::build_from_paths;
use crate::repo::Repo;
use crate::Error;
pub(crate) use repo::RepoConfig;

mod repo;

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct ConfigPath {
    pub cache: Utf8PathBuf,
    pub config: Utf8PathBuf,
    pub data: Utf8PathBuf,
    pub db: Utf8PathBuf,
    pub run: Utf8PathBuf,
}

impl ConfigPath {
    fn new(name: &str, prefix: &str, create: bool) -> crate::Result<ConfigPath> {
        let home = env::var("HOME").ok().unwrap_or_else(|| "/root".to_string());
        let (config, cache, data, db, run): (
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        );

        // prefix a given path
        let prefixed = |p: Utf8PathBuf| -> Utf8PathBuf {
            match prefix.is_empty() {
                true => p,
                false => Utf8PathBuf::from(prefix).join(p.strip_prefix("/").unwrap_or(&p)),
            }
        };

        // pull user config from $XDG_CONFIG_HOME, otherwise $HOME/.config
        let user_config: Utf8PathBuf = match env::var("XDG_CONFIG_HOME") {
            Ok(x) => prefixed(build_from_paths!(&x, name)),
            Err(_) => prefixed(build_from_paths!(&home, ".config", name)),
        };

        let system_config = prefixed(Utf8PathBuf::from(format!("/etc/{name}")));

        // determine if user config or system config will be used
        config = match (user_config.exists(), system_config.exists() || home == "/root") {
            (false, true) => {
                cache = prefixed(Utf8PathBuf::from(format!("/var/cache/{name}")));
                data = prefixed(Utf8PathBuf::from(format!("/usr/share/{name}")));
                db = prefixed(Utf8PathBuf::from(format!("/var/db/{name}")));
                run = prefixed(Utf8PathBuf::from(format!("/run/{name}")));
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
    pub fn new(name: &str, prefix: &str, create: bool) -> crate::Result<Config> {
        let path = ConfigPath::new(name, prefix, create)?;
        let repos = repo::Config::new(&path.config, &path.db, create)?;
        repos.finalize()?;
        let config = Config { path, repos };
        Config::make_current(config.clone());
        Ok(config)
    }

    pub fn current() -> Arc<Config> {
        CURRENT_CONFIG.read().unwrap().clone()
    }

    fn make_current(config: Config) {
        *CURRENT_CONFIG.write().unwrap() = Arc::new(config);
    }

    // Note that repo references can't be returned since the underlying map structure alters them
    // during mutations causing references to change.

    /// Add local repo from a filesystem path.
    pub fn add_repo_path(&mut self, name: &str, priority: i32, path: &str) -> crate::Result<Repo> {
        let r = self.repos.add_path(name, priority, path)?;
        r.finalize()?;
        self.repos.insert(name, r.clone(), true);
        Config::make_current(self.clone());
        Ok(r)
    }

    /// Add external repo from a URI.
    pub fn add_repo_uri(&mut self, name: &str, priority: i32, uri: &str) -> crate::Result<Repo> {
        let r = self.repos.add_uri(name, priority, uri)?;
        r.finalize()?;
        self.repos.insert(name, r.clone(), false);
        Config::make_current(self.clone());
        Ok(r)
    }

    /// Create a new repo.
    pub fn create_repo(&mut self, name: &str, priority: i32) -> crate::Result<Repo> {
        let r = self.repos.create(name, priority)?;
        r.finalize()?;
        self.repos.insert(name, r.clone(), false);
        Config::make_current(self.clone());
        Ok(r)
    }

    /// Remove configured repos.
    pub fn del_repos<S: AsRef<str>>(&mut self, repos: &[S], clean: bool) -> crate::Result<()> {
        self.repos.del(repos, clean)?;
        self.repos.finalize()?;
        Config::make_current(self.clone());
        Ok(())
    }

    /// Load repos from a given path to a portage-compatible repos.conf directory or file.
    pub fn load_repos_conf<P: AsRef<Utf8Path>>(&mut self, path: P) -> crate::Result<Vec<Repo>> {
        let path = path.as_ref();
        let files: Vec<_> = match path.read_dir() {
            Ok(entries) => Ok(entries.filter_map(|d| d.ok()).map(|d| d.path()).collect()),
            // TODO: switch to `e.kind() == ErrorKind::NotADirectory` on rust stabilization
            // https://github.com/rust-lang/rust/issues/86442
            Err(e) if e.raw_os_error() == Some(20) => Ok(vec![PathBuf::from(path)]),
            Err(e) => Err(Error::Config(format!("failed reading repos.conf: {path:?}: {e}"))),
        }?;

        // copy original config that is reverted to if an error occurs
        let orig_config = Config::current().as_ref().clone();
        let mut repos = vec![];

        for f in files {
            Ini::load_from_file(&f)
                .map_err(|e| Error::Config(format!("invalid repos.conf file: {f:?}: {e}")))
                .and_then(|ini| {
                    for (name, settings) in ini.iter().filter_map(|(section, p)| {
                        match section {
                            Some(s) if s != "DEFAULT" => Some((s, p)),
                            _ => None,
                        }
                    }) {
                        // pull supported fields from config
                        let priority = settings.get("priority")
                            .unwrap_or("0")
                            .parse()
                            .unwrap_or(0);
                        let path = settings.get("location").ok_or_else(|| {
                            Error::Config(format!(
                                "invalid repos.conf file: {f:?}: missing location field for {name:?} repo"
                            ))
                        })?;

                        let r = self.repos.add_path(name, priority, path)?;
                        self.repos.insert(name, r.clone(), true);
                        repos.push(r);
                    }
                    Ok(())
                })?;
        }

        Config::make_current(self.clone());
        for repo in &repos {
            if let Err(e) = repo.finalize() {
                // revert to previous config without any repos from repos.conf files
                Config::make_current(orig_config);
                return Err(e);
            }
        }

        Ok(repos)
    }

    /// Create a new temporary ebuild repo.
    #[cfg(test)]
    pub(crate) fn temp_repo(
        &mut self,
        name: &str,
        priority: i32,
    ) -> crate::Result<(crate::repo::ebuild::TempRepo, Arc<crate::repo::ebuild::Repo>)> {
        let (temp_repo, r) = self.repos.create_temp(name, priority)?;
        r.finalize()?;
        self.repos.insert(name, r.clone(), false);
        Config::make_current(self.clone());
        let repo = self.repos.get(name).unwrap().as_ebuild().unwrap();
        Ok((temp_repo, repo.clone()))
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
        assert_eq!(config.path.cache, Utf8PathBuf::from("/cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/config/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix", false).unwrap();
        assert_eq!(config.path.cache, Utf8PathBuf::from("/prefix/cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/prefix/config/pkgcraft"));

        env::remove_var("XDG_CACHE_HOME");
        env::remove_var("XDG_CONFIG_HOME");

        // XDG var is unset and HOME is set
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(config.path.cache, Utf8PathBuf::from("/home/user/.cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/home/user/.config/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix", false).unwrap();
        assert_eq!(config.path.cache, Utf8PathBuf::from("/prefix/home/user/.cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/prefix/home/user/.config/pkgcraft"));
        env::remove_var("HOME");

        // XDG var and HOME are unset
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(config.path.config, Utf8PathBuf::from("/etc/pkgcraft"));
    }
}
