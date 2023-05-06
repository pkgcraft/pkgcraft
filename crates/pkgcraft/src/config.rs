use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use ini::Ini;
use serde::{Deserialize, Serialize};

use crate::eapi::Eapi;
use crate::macros::build_from_paths;
use crate::repo::ebuild::Repo as EbuildRepo;
use crate::repo::ebuild_temp::Repo as TempRepo;
use crate::repo::Repo;
use crate::Error;
pub(crate) use repo::RepoConfig;

mod repo;

/// Set types of configured repos
#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum RepoSetType {
    All,
    Ebuild,
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
    fn new(name: &str, prefix: &str) -> Self {
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
            if prefix.is_empty() {
                p
            } else {
                Utf8PathBuf::from(prefix).join(p.strip_prefix("/").unwrap_or(&p))
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

        Self {
            cache,
            config,
            data,
            db,
            run,
            tmp,
        }
    }

    /// Create all config paths.
    fn create_paths(&self) -> crate::Result<()> {
        for path in [&self.cache, &self.config, &self.data, &self.db, &self.run] {
            fs::create_dir_all(path).map_err(|e| Error::Config(e.to_string()))?;
        }
        Ok(())
    }
}

/// System config
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Config {
    pub path: ConfigPath,
    pub repos: repo::Config,
}

impl Config {
    pub fn new(name: &str, prefix: &str) -> Self {
        let path = ConfigPath::new(name, prefix);
        Config { path, ..Default::default() }
    }

    /// Load repos from toml files in the related repos config dir.
    pub fn load_repos(&mut self) -> crate::Result<()> {
        self.repos = repo::Config::new(&self.path.config, &self.path.db)?;
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

        let mut repos = vec![];

        for f in files {
            Ini::load_from_file(&f)
                .map_err(|e| Error::Config(format!("invalid repos.conf file: {f:?}: {e}")))
                .and_then(|ini| {
                    for (name, settings) in ini.iter().filter_map(|(section, p)| match section {
                        Some(s) if s != "DEFAULT" => Some((s, p)),
                        _ => None,
                    }) {
                        // pull supported fields from config
                        let priority = settings.get("priority").unwrap_or("0").parse().unwrap_or(0);
                        let path = settings.get("location").ok_or_else(|| {
                            Error::Config(format!(
                                "invalid repos.conf file: {f:?}: missing location field: {name}"
                            ))
                        })?;

                        repos.push(Repo::from_path(name, priority, path, false)?);
                    }
                    Ok(())
                })?;
        }

        if !repos.is_empty() {
            // add repos to config
            self.repos.extend(&repos)?;
        }

        repos.sort();
        Ok(repos)
    }

    /// Create all config-related paths.
    pub fn create_paths(&self) -> crate::Result<()> {
        self.path.create_paths()?;
        self.repos.create_paths()?;
        Ok(())
    }

    // Note that repo references can't be returned since the underlying map structure alters them
    // during mutations causing references to change.

    /// Add local repo from a filesystem path.
    pub fn add_repo_path<P: AsRef<Utf8Path>>(
        &mut self,
        name: &str,
        priority: i32,
        path: P,
    ) -> crate::Result<Repo> {
        let r = Repo::from_path(name, priority, path, false)?;
        self.add_repo(&r)?;
        Ok(r)
    }

    /// Add external repo from a URI.
    pub fn add_repo_uri(&mut self, name: &str, priority: i32, uri: &str) -> crate::Result<Repo> {
        let r = self.repos.add_uri(name, priority, uri)?;
        self.add_repo(&r)?;
        Ok(r)
    }

    /// Add a repo to the config.
    pub fn add_repo(&mut self, repo: &Repo) -> crate::Result<()> {
        self.repos.extend([repo])
    }

    /// Create a new repo.
    pub fn create_repo(&mut self, name: &str, priority: i32) -> crate::Result<Repo> {
        let r = self.repos.create(name, priority)?;
        self.add_repo(&r)?;
        Ok(r)
    }

    /// Remove configured repos.
    pub fn del_repos<S: AsRef<str>>(&mut self, repos: &[S], clean: bool) -> crate::Result<()> {
        // TODO: verify repos to be removed aren't required by remaining repos
        self.repos.del(repos, clean)?;
        Ok(())
    }

    /// Create a new temporary ebuild repo.
    pub fn temp_repo(
        &mut self,
        name: &str,
        priority: i32,
        eapi: Option<&Eapi>,
    ) -> crate::Result<(TempRepo, Arc<EbuildRepo>)> {
        let (temp_repo, r) = self.repos.create_temp(name, priority, eapi)?;
        self.add_repo(&r)?;
        let repo = r.as_ebuild().expect("invalid ebuild repo: {name}");
        Ok((temp_repo, repo.clone()))
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use tempfile::tempdir;
    use tracing_test::traced_test;

    use crate::macros::*;
    use crate::repo::Repository;
    use crate::test::{assert_ordered_eq, TEST_DATA};

    use super::*;

    #[test]
    fn test_config() {
        env::set_var("XDG_CACHE_HOME", "/cache");
        env::set_var("XDG_CONFIG_HOME", "/config");
        env::set_var("XDG_RUNTIME_DIR", "/run/user/4321");
        env::set_var("HOME", "/home/user");

        // XDG vars and HOME are set
        let config = Config::new("pkgcraft", "");
        assert_eq!(config.path.cache, Utf8PathBuf::from("/cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/config/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/run/user/4321/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix");
        assert_eq!(config.path.cache, Utf8PathBuf::from("/prefix/cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/prefix/config/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/prefix/run/user/4321/pkgcraft"));

        env::remove_var("XDG_CACHE_HOME");
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("XDG_RUNTIME_DIR");

        // XDG vars are unset and HOME is set
        let config = Config::new("pkgcraft", "");
        assert_eq!(config.path.cache, Utf8PathBuf::from("/home/user/.cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/home/user/.config/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/home/user/.cache/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix");
        assert_eq!(config.path.cache, Utf8PathBuf::from("/prefix/home/user/.cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/prefix/home/user/.config/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/prefix/home/user/.cache/pkgcraft"));
        env::remove_var("HOME");

        // XDG vars and HOME are unset
        let config = Config::new("pkgcraft", "");
        assert_eq!(config.path.cache, Utf8PathBuf::from("/var/cache/pkgcraft"));
        assert_eq!(config.path.config, Utf8PathBuf::from("/etc/pkgcraft"));
        assert_eq!(config.path.run, Utf8PathBuf::from("/run/pkgcraft"));
    }

    #[traced_test]
    #[test]
    fn test_load_repos_conf() {
        let mut config = Config::new("pkgcraft", "");
        let tmpdir = tempdir().unwrap();
        let conf_path = tmpdir.path().join("repos.conf");
        let path = conf_path.to_str().unwrap();

        // nonexistent
        let r = config.load_repos_conf("nonexistent");
        assert_err_re!(r, "failed reading repos.conf");

        // invalid ini format
        let data = indoc::indoc! {r#"
            [DEFAULT]
            main-repo = gentoo

            [overlay
            location = /path/to/overlay
        "#};
        fs::write(path, data).unwrap();
        let r = config.load_repos_conf(path);
        assert_err_re!(r, "invalid repos.conf file");

        // invalid ini format
        let data = indoc::indoc! {r#"
            [DEFAULT]
            main-repo = gentoo

            [overlay]
        "#};
        fs::write(path, data).unwrap();
        let r = config.load_repos_conf(path);
        assert_err_re!(r, "missing location field: overlay");

        // empty
        fs::write(path, "").unwrap();
        let repos = config.load_repos_conf(path).unwrap();
        assert!(repos.is_empty());

        // single repo
        let t1 = TempRepo::new("test", None, None).unwrap();
        let data = indoc::formatdoc! {r#"
            [a]
            location = {}
        "#, t1.path()};
        fs::write(path, data).unwrap();
        let repos = config.load_repos_conf(path).unwrap();
        assert_ordered_eq(repos.iter().map(|r| r.id()), ["a"]);

        // multiple, prioritized repos
        let mut config = Config::new("pkgcraft", "");
        let t2 = TempRepo::new("r2", None, None).unwrap();
        let data = indoc::formatdoc! {r#"
            [b]
            location = {}
            [c]
            location = {}
            priority = 1
        "#, t1.path(), t2.path()};
        fs::write(path, data).unwrap();
        let repos = config.load_repos_conf(path).unwrap();
        assert_ordered_eq(repos.iter().map(|r| r.id()), ["c", "b"]);

        // multiple config files in a specified directory
        let mut config = Config::new("pkgcraft", "");
        let t3 = TempRepo::new("r3", None, None).unwrap();
        let tmpdir = tempdir().unwrap();
        let conf_dir = tmpdir.path();
        let data = indoc::formatdoc! {r#"
            [r1]
            location = {}
        "#, t1.path()};
        fs::write(conf_dir.join("r1.conf"), data).unwrap();
        let data = indoc::formatdoc! {r#"
            [r2]
            location = {}
            priority = -1
        "#, t2.path()};
        fs::write(conf_dir.join("r2.conf"), data).unwrap();
        let data = indoc::formatdoc! {r#"
            [r3]
            location = {}
            priority = 1
        "#, t3.path()};
        fs::write(conf_dir.join("r3.conf"), data).unwrap();
        let repos = config.load_repos_conf(conf_dir.to_str().unwrap()).unwrap();
        assert_ordered_eq(repos.iter().map(|r| r.id()), ["r3", "r1", "r2"]);

        // reloading existing repo fails
        let data = indoc::formatdoc! {r#"
            [r1]
            location = {}
        "#, t1.path()};
        fs::write(path, data).unwrap();
        let r = config.load_repos_conf(path);
        assert_err_re!(r, "existing repos: r1");

        // reloading existing repo using a different id fails
        let data = indoc::formatdoc! {r#"
            [r4]
            location = {}
        "#, t1.path()};
        fs::write(path, data).unwrap();
        let r = config.load_repos_conf(path);
        assert_err_re!(r, "existing repos: r4");

        // nonexistent masters causes finalization failure
        let mut config = Config::new("pkgcraft", "");
        let repos_path = TEST_DATA.path.join("repos");
        let data = indoc::formatdoc! {r#"
            [primary]
            location = {repos_path}/dependent-primary
            [nonexistent]
            location = {repos_path}/dependent-nonexistent
        "#};
        fs::write(path, data).unwrap();
        let r = config.load_repos_conf(path);
        assert_err_re!(r, "^.* unconfigured repos: nonexistent1, nonexistent2$");
    }
}
