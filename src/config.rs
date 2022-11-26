use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use ini::Ini;
use serde::{Deserialize, Serialize};

use crate::macros::build_from_paths;
use crate::repo::ebuild::{Repo as EbuildRepo, TempRepo};
use crate::repo::{Repo, Repository};
use crate::Error;
pub(crate) use repo::RepoConfig;

mod repo;

/// Set types of configured repos
#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum RepoSetType {
    AllRepos,
    EbuildRepos,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct ConfigPath {
    pub cache: Utf8PathBuf,
    pub config: Utf8PathBuf,
    pub data: Utf8PathBuf,
    pub db: Utf8PathBuf,
    pub run: Utf8PathBuf,
    pub tmp: Utf8PathBuf,
}

impl ConfigPath {
    fn new(name: &str, prefix: &str, create: bool) -> crate::Result<ConfigPath> {
        let home = env::var("HOME").ok().unwrap_or_else(|| "/root".to_string());
        let (config, cache, data, db, run, tmp): (
            Utf8PathBuf,
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
                tmp = prefixed(Utf8PathBuf::from(format!("/var/tmp/{name}")));
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

                // pull user runtime path from $XDG_RUNTIME_DIR, otherwise use the cache directory.
                run = match env::var("XDG_RUNTIME_DIR") {
                    Ok(x) => prefixed(build_from_paths!(&x, name)),
                    Err(_) => cache.clone(),
                };

                db = data.clone();
                tmp = cache.clone();
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
            tmp,
        })
    }
}

/// System config
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    pub path: ConfigPath,
    pub repos: repo::Config,
}

impl Config {
    pub fn new(name: &str, prefix: &str, create: bool) -> crate::Result<Config> {
        let mut config = Config {
            path: ConfigPath::new(name, prefix, create)?,
            ..Default::default()
        };
        config.repos = repo::Config::new(&config.path.config, &config.path.db, create)?;
        for (_, repo) in &config.repos {
            repo.finalize(&config)?;
        }
        Ok(config)
    }

    // Note that repo references can't be returned since the underlying map structure alters them
    // during mutations causing references to change.

    /// Add local repo from a filesystem path.
    pub fn add_repo_path(&mut self, name: &str, priority: i32, path: &str) -> crate::Result<Repo> {
        let r = self.repos.add_path(name, priority, path)?;
        self.add_repo(&r, true)?;
        Ok(r)
    }

    /// Add external repo from a URI.
    pub fn add_repo_uri(&mut self, name: &str, priority: i32, uri: &str) -> crate::Result<Repo> {
        let r = self.repos.add_uri(name, priority, uri)?;
        self.add_repo(&r, false)?;
        Ok(r)
    }

    /// Add a repo to the config.
    pub fn add_repo(&mut self, repo: &Repo, external: bool) -> crate::Result<()> {
        repo.finalize(self)?;
        self.repos.insert(repo.id(), repo.clone(), external);
        Ok(())
    }

    /// Create a new repo.
    pub fn create_repo(&mut self, name: &str, priority: i32) -> crate::Result<Repo> {
        let r = self.repos.create(name, priority)?;
        self.add_repo(&r, false)?;
        Ok(r)
    }

    /// Remove configured repos.
    pub fn del_repos<S: AsRef<str>>(&mut self, repos: &[S], clean: bool) -> crate::Result<()> {
        // TODO: verify repos to be removed aren't required by remaining repos
        self.repos.del(repos, clean)?;
        Ok(())
    }

    /// Load repos from a portage-compatible repos.conf directory or file.
    pub fn load_repos_conf<P: AsRef<Utf8Path>>(&mut self, path: P) -> crate::Result<Vec<Repo>> {
        let path = path.as_ref();
        let files: Vec<_> = match path.read_dir() {
            Ok(entries) => Ok(entries.filter_map(|d| d.ok()).map(|d| d.path()).collect()),
            // TODO: switch to `e.kind() == ErrorKind::NotADirectory` on rust stabilization
            // https://github.com/rust-lang/rust/issues/86442
            Err(e) if e.raw_os_error() == Some(20) => Ok(vec![PathBuf::from(path)]),
            Err(e) => Err(Error::Config(format!("failed reading repos.conf: {path:?}: {e}"))),
        }?;

        // copy original repos config that is reverted to if an error occurs
        let orig_repos = self.repos.clone();
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
                        repos.push((name.to_string(), r));
                    }
                    Ok(())
                })?;
        }

        if !repos.is_empty() {
            // add repos to config
            self.repos.extend(&repos, true);

            // verify new repos
            for (_name, repo) in &repos {
                if let Err(e) = repo.finalize(self) {
                    // revert to previous repos
                    self.repos = orig_repos;
                    return Err(e);
                }
            }
        }

        repos.sort_by(|(_, r1), (_, r2)| r1.cmp(r2));
        Ok(repos.into_iter().map(|(_, r)| r).collect())
    }

    /// Create a new temporary ebuild repo.
    pub fn temp_repo(
        &mut self,
        name: &str,
        priority: i32,
    ) -> crate::Result<(TempRepo, Arc<EbuildRepo>)> {
        let (temp_repo, r) = self.repos.create_temp(name, priority)?;
        r.finalize(self)?;
        self.repos.insert(name, r, false);
        match self.repos.get(name) {
            Some(Repo::Ebuild(r)) => Ok((temp_repo, r.clone())),
            _ => panic!("unknown temp repo: {}", name),
        }
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
        env::set_var("XDG_RUNTIME_DIR", "/run/user/4321");
        env::set_var("HOME", "/home/user");

        // XDG vars and HOME are set
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(config.path.cache, Utf8PathBuf::from("/cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/config/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/run/user/4321/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix", false).unwrap();
        assert_eq!(config.path.cache, Utf8PathBuf::from("/prefix/cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/prefix/config/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/prefix/run/user/4321/pkgcraft"));

        env::remove_var("XDG_CACHE_HOME");
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("XDG_RUNTIME_DIR");

        // XDG vars are unset and HOME is set
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(config.path.cache, Utf8PathBuf::from("/home/user/.cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/home/user/.config/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/home/user/.cache/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix", false).unwrap();
        assert_eq!(config.path.cache, Utf8PathBuf::from("/prefix/home/user/.cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/prefix/home/user/.config/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/prefix/home/user/.cache/pkgcraft"));
        env::remove_var("HOME");

        // XDG vars and HOME are unset
        let config = Config::new("pkgcraft", "", false).unwrap();
        assert_eq!(config.path.cache, Utf8PathBuf::from("/var/cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/etc/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/run/pkgcraft"));
    }
}
