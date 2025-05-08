use std::env;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

use crate::macros::build_path;
use crate::repo::{Repo, RepoFormat, Repository};
use crate::utils::find_existing_path;
use crate::{Error, shell};
pub(crate) use repo::RepoConfig;

mod portage;
mod repo;
pub(crate) use repo::ConfigRepos;

const PORTAGE_CONFIG_PATHS: &[&str] = &["/etc/portage", "/usr/share/portage/config"];

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
            Ok(x) => prefixed(build_path!(&x, name)),
            Err(_) => prefixed(build_path!(&home, ".config", name)),
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
                    Ok(x) => prefixed(build_path!(&x, name)),
                    Err(_) => prefixed(build_path!(&home, ".cache", name)),
                };

                // pull user data path from $XDG_DATA_HOME, otherwise $HOME/.local/share
                data = match env::var("XDG_DATA_HOME") {
                    Ok(x) => prefixed(build_path!(&x, name)),
                    Err(_) => prefixed(build_path!(&home, ".local", "share", name)),
                };

                // pull user runtime path from $XDG_RUNTIME_DIR, otherwise use the cache directory.
                run = match env::var("XDG_RUNTIME_DIR") {
                    Ok(x) => prefixed(build_path!(&x, name)),
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
}

#[derive(Debug, Default, Clone)]
pub struct Settings {
    options: IndexSet<String>,
}

impl Settings {
    pub fn options(&self) -> &IndexSet<String> {
        &self.options
    }
}

mod sealed {
    use super::{ConfigPath, ConfigRepos, Settings};
    use std::sync::Arc;

    pub trait Config: Default + std::fmt::Debug {
        fn repos(&self) -> &ConfigRepos;

        fn path(&self) -> &ConfigPath;

        fn settings(&self) -> &Arc<Settings>;
    }
}

#[derive(Debug, Default)]
pub struct ConfigInner {
    path: ConfigPath,
    pub(crate) repos: ConfigRepos,
    settings: Arc<Settings>,
    /// Flag used to denote when config files have been loaded.
    loaded: bool,
    // TODO: Remove it later
    pool: Arc<shell::BuildPool>,
}

impl ConfigInner {
    /// Load user or system config files, if none are found revert to loading portage files.
    pub fn load(&mut self) -> crate::Result<()> {
        if !self.loaded {
            if let Ok(value) = env::var("PKGCRAFT_CONFIG") {
                self.load_path(&value)?;
            } else {
                self.settings = Default::default();
                self.repos =
                    ConfigRepos::new(&self.path.config, &self.path.db, &self.settings)?;
            }
        }

        self.loaded = true;
        Ok(())
    }

    /// Load config files from a given path.
    pub fn load_path(&mut self, path: &str) -> crate::Result<()> {
        if !self.loaded && !path.is_empty() && self.path.config.exists() {
            self.repos = ConfigRepos::new(&self.path.config, &self.path.db, &self.settings)?;
        };

        self.loaded = true;
        Ok(())
    }

    /// Load portage config files from a given directory, falling back to the default locations.
    pub fn load_portage_conf(&mut self, path: Option<&str>) -> crate::Result<()> {
        // use specified path or use fallbacks
        let config_dirs = if let Some(value) = path {
            &[value]
        } else {
            PORTAGE_CONFIG_PATHS
        };

        let paths = config_dirs
            .iter()
            .map(|s| Utf8Path::new(s).join("repos.conf"));

        // use the repos.conf file that exists
        let repos_conf = if let Some(p) = find_existing_path(paths) {
            p
        } else {
            let err = if let Some(s) = path {
                Error::Config(format!("nonexistent portage config path: {s}"))
            } else {
                Error::ConfigMissing("no portage config found".to_string())
            };
            return Err(err);
        };

        let repos = portage::load_repos_conf(repos_conf)?;
        if !repos.is_empty() {
            self.repos.extend(repos, &self.settings)?;
        }

        self.loaded = true;
        Ok(())
    }
}

impl sealed::Config for ConfigInner {
    fn repos(&self) -> &ConfigRepos {
        &self.repos
    }

    fn path(&self) -> &ConfigPath {
        &self.path
    }

    fn settings(&self) -> &Arc<Settings> {
        &self.settings
    }
}

/*
#[derive(Debug, Default)]
struct ConfigFinalized {
    inner: Arc<ConfigInner>,
    pool: Arc<shell::BuildPool>,
}

impl sealed::Config for ConfigFinalized {
    fn repos(&self) -> &ConfigRepos {
        self.inner.repos()
    }

    fn path(&self) -> &ConfigPath {
        self.inner.path()
    }

    fn settings(&self) -> &Arc<Settings> {
        self.inner.settings()
    }
}
*/

/// System config
#[derive(Debug, Default)]
pub struct Config<C: sealed::Config = ConfigInner> {
    inner: C,
}

impl<C: sealed::Config> From<&Config<C>> for Arc<Settings> {
    fn from(config: &Config<C>) -> Self {
        config.settings().clone()
    }
}

// Accessors for the main fields
impl<C: sealed::Config> Config<C> {
    pub fn repos(&self) -> &ConfigRepos {
        self.inner.repos()
    }

    pub fn path(&self) -> &ConfigPath {
        self.inner.path()
    }

    pub fn settings(&self) -> &Arc<Settings> {
        self.inner.settings()
    }
}

// Accessor for repos
impl<C: sealed::Config> Config<C> {
    /// Get a repo from the config.
    pub fn get_repo<S: AsRef<str>>(&self, key: S) -> crate::Result<&Repo> {
        let repo = self.repos().get(key)?;

        Ok(repo)
    }

    /// Determine if the config has all named repos loaded.
    fn has_repos<I>(&self, repos: I) -> bool
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        repos.into_iter().all(|x| self.repos().get(x).is_ok())
    }
}

impl Config<ConfigInner> {
    pub fn new(name: &str, prefix: &str) -> Self {
        let path = ConfigPath::new(name, prefix);
        let inner = ConfigInner { path, ..Default::default() };
        Config { inner }
    }

    /// Load user or system config files, if none are found revert to loading portage files.
    pub fn load(&mut self) -> crate::Result<()> {
        self.inner.load()
    }

    /// Load config files from a given path.
    pub fn load_path(&mut self, path: &str) -> crate::Result<()> {
        self.inner.load_path(path)
    }

    /// Load portage config files from a given directory, falling back to the default locations.
    pub fn load_portage_conf(&mut self, path: Option<&str>) -> crate::Result<()> {
        self.inner.load_portage_conf(path)
    }

    /// Add local repo from a filesystem path.
    pub fn add_repo_path<S: AsRef<str>, P: AsRef<Utf8Path>>(
        &mut self,
        name: S,
        path: P,
        priority: i32,
    ) -> crate::Result<Repo> {
        let r = Repo::from_path(name, path, priority)?;
        self.add_repo(r)
    }

    /// Add local repo of a specific format from a filesystem path.
    pub fn add_format_repo_path<S: AsRef<str>, P: AsRef<Utf8Path>>(
        &mut self,
        name: S,
        path: P,
        priority: i32,
        format: RepoFormat,
    ) -> crate::Result<Repo> {
        let r = format.from_path(name, path, priority)?;
        self.add_repo(r)
    }

    /// Add local repo from a potentially nested filesystem path.
    pub fn add_nested_repo_path<P: AsRef<Utf8Path>>(
        &mut self,
        path: P,
        priority: i32,
    ) -> crate::Result<Repo> {
        let r = Repo::from_nested_path(path, priority)?;
        self.add_repo(r)
    }

    /// Add local repo of a specific format from a potentially nested filesystem path.
    pub fn add_format_repo_nested_path<P: AsRef<Utf8Path>>(
        &mut self,
        path: P,
        priority: i32,
        format: RepoFormat,
    ) -> crate::Result<Repo> {
        let path = path.as_ref();
        match format.from_nested_path(path, priority) {
            Err(Error::NotARepo { .. }) => {
                Err(Error::InvalidValue(format!("invalid {format} repo: {path}")))
            }
            Err(e) => Err(e),
            Ok(r) => self.add_repo(r),
        }
    }

    /// Add a repo to the config.
    pub fn add_repo<T>(&mut self, value: T) -> crate::Result<Repo>
    where
        T: TryInto<Repo>,
        Error: From<T::Error>,
    {
        let repo: Repo = value.try_into()?;

        // automatically load repo deps if possible
        if let Err(Error::NonexistentRepoMasters { repos }) = repo.finalize(self) {
            // load system config
            self.load()?;

            // if repos are still missing, try loading from the parent dir
            if !self.has_repos(&repos) {
                if let Some(parent) = repo.path().parent() {
                    for name in &repos {
                        let path = parent.join(name);
                        if self
                            .add_format_repo_path(name, &path, repo.priority(), repo.format())
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }

            repo.finalize(self)?;
        }

        let settings = self.settings().clone();

        self.inner.repos.extend([repo.clone()], &settings)?;

        Ok(repo)
    }

    /// Return the mutable repos configuration.
    pub fn repos_mut(&mut self) -> &mut ConfigRepos {
        &mut self.inner.repos
    }

    // TODO: Move to ConfigFinalized once repo is generic over Config.

    /// Finalize the config repos and start the build pool.
    pub fn finalize(&self) -> crate::Result<()> {
        // finalize repos
        for (id, repo) in &self.inner.repos {
            repo.finalize(self)
                .map_err(|e| Error::Config(format!("{id}: {e}")))?;
        }

        // start the build pool
        self.inner.pool.start(self.inner.repos.clone())?;

        Ok(())
    }

    /// Return the build pool for the config.
    pub fn pool(&self) -> &Arc<shell::BuildPool> {
        &self.inner.pool
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;
    use tracing_test::traced_test;

    use crate::repo::Repository;
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::test::*;

    use super::*;

    #[test]
    fn config() {
        unsafe {
            env::set_var("XDG_CACHE_HOME", "/cache");
            env::set_var("XDG_CONFIG_HOME", "/config");
            env::set_var("XDG_RUNTIME_DIR", "/run/user/4321");
            env::set_var("HOME", "/home/user");
        }

        // XDG vars and HOME are set
        let config = Config::new("pkgcraft", "");
        assert_eq!(config.path().cache, Utf8PathBuf::from("/cache/pkgcraft"));
        assert_eq!(config.path().config, Utf8PathBuf::from("/config/pkgcraft"));
        assert_eq!(config.path().run, Utf8PathBuf::from("/run/user/4321/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix");
        assert_eq!(config.path().cache, Utf8PathBuf::from("/prefix/cache/pkgcraft"));
        assert_eq!(config.path().config, Utf8PathBuf::from("/prefix/config/pkgcraft"));
        assert_eq!(config.path().run, Utf8PathBuf::from("/prefix/run/user/4321/pkgcraft"));

        unsafe {
            env::remove_var("XDG_CACHE_HOME");
            env::remove_var("XDG_CONFIG_HOME");
            env::remove_var("XDG_RUNTIME_DIR");
        }

        // XDG vars are unset and HOME is set
        let config = Config::new("pkgcraft", "");
        assert_eq!(config.path().cache, Utf8PathBuf::from("/home/user/.cache/pkgcraft"));
        assert_eq!(config.path().config, Utf8PathBuf::from("/home/user/.config/pkgcraft"));
        assert_eq!(config.path().run, Utf8PathBuf::from("/home/user/.cache/pkgcraft"));

        // prefix
        let config = Config::new("pkgcraft", "/prefix");
        assert_eq!(
            config.path().cache,
            Utf8PathBuf::from("/prefix/home/user/.cache/pkgcraft")
        );
        assert_eq!(
            config.path().config,
            Utf8PathBuf::from("/prefix/home/user/.config/pkgcraft")
        );
        assert_eq!(config.path().run, Utf8PathBuf::from("/prefix/home/user/.cache/pkgcraft"));
        unsafe { env::remove_var("HOME") };

        // XDG vars and HOME are unset
        let config = Config::new("pkgcraft", "");
        assert_eq!(config.path().cache, Utf8PathBuf::from("/var/cache/pkgcraft"));
        assert_eq!(config.path().config, Utf8PathBuf::from("/etc/pkgcraft"));
        assert_eq!(config.path().run, Utf8PathBuf::from("/run/pkgcraft"));
    }

    #[traced_test]
    #[test]
    fn load_portage_conf() {
        let mut config = Config::new("pkgcraft", "");
        let tmpdir = tempdir().unwrap();
        let conf_path = tmpdir.path().to_str().unwrap();
        let path = tmpdir.path().join("repos.conf");
        let path = path.to_str().unwrap();
        let data = test_data();
        let repos_dir = data.path().join("repos");

        // nonexistent
        let r = config.load_portage_conf(Some("unknown/path"));
        assert_err_re!(r, "nonexistent portage config path: unknown/path");

        // invalid ini format
        let data = indoc::indoc! {r#"
            [DEFAULT]
            main-repo = gentoo

            [overlay
            location = /path/to/overlay
        "#};
        fs::write(path, data).unwrap();
        let r = config.load_portage_conf(Some(conf_path));
        assert_err_re!(r, "invalid repos.conf file");

        // invalid ini format
        let data = indoc::indoc! {r#"
            [DEFAULT]
            main-repo = gentoo

            [overlay]
        "#};
        fs::write(path, data).unwrap();
        assert!(config.load_portage_conf(Some(conf_path)).is_ok());
        assert_logs_re!(".+: missing location field: overlay");

        // empty
        fs::write(path, "").unwrap();
        assert!(config.load_portage_conf(Some(conf_path)).is_ok());
        assert!(config.repos().is_empty());

        // single repo
        let t1 = EbuildRepoBuilder::new().build().unwrap();
        let data = indoc::formatdoc! {r#"
            [a]
            location = {}
        "#, t1.path()};
        fs::write(path, data).unwrap();
        config.load_portage_conf(Some(conf_path)).unwrap();
        assert_ordered_eq!(config.repos().iter().map(|(_, r)| r.id()), ["a"]);

        // single repo with unsupported EAPI
        let mut config = Config::new("pkgcraft", "");
        let data = indoc::formatdoc! {r#"
            [unsupported]
            location = {repos_dir}/invalid/unsupported-eapi
        "#};
        fs::write(path, data).unwrap();
        assert!(config.load_portage_conf(Some(conf_path)).is_ok());
        assert_logs_re!(".+: invalid repo: unsupported: profiles/eapi: unsupported EAPI: 0");

        // invalid and valid repos
        let mut config = Config::new("pkgcraft", "");
        let data = indoc::formatdoc! {r#"
            [unsupported]
            location = {repos_dir}/invalid/unsupported-eapi
            [empty]
            location = {repos_dir}/valid/empty
        "#};
        fs::write(path, data).unwrap();
        assert!(config.load_portage_conf(Some(conf_path)).is_ok());
        assert_ordered_eq!(config.repos().iter().map(|(_, r)| r.id()), ["empty"]);

        // multiple, prioritized repos
        let mut config = Config::new("pkgcraft", "");
        let t2 = EbuildRepoBuilder::new().name("r2").build().unwrap();
        let data = indoc::formatdoc! {r#"
            [b]
            location = {}
            [c]
            location = {}
            priority = 1
        "#, t1.path(), t2.path()};
        fs::write(path, data).unwrap();
        config.load_portage_conf(Some(conf_path)).unwrap();
        assert_ordered_eq!(config.repos().iter().map(|(_, r)| r.id()), ["c", "b"]);

        // reloading existing repo using a different id succeeds
        let data = indoc::formatdoc! {r#"
            [r4]
            location = {}
        "#, t1.path()};
        fs::write(path, data).unwrap();
        config.load_portage_conf(Some(conf_path)).unwrap();
        assert_ordered_eq!(config.repos().iter().map(|(_, r)| r.id()), ["c", "b", "r4"]);

        // nonexistent masters causes finalization failure
        let mut config = Config::new("pkgcraft", "");
        let data = indoc::formatdoc! {r#"
            [primary]
            location = {repos_dir}/valid/primary
            [nonexistent]
            location = {repos_dir}/invalid/nonexistent-masters
        "#};
        fs::write(path, data).unwrap();
        config.load_portage_conf(Some(conf_path)).unwrap();
        let r = config.finalize();
        assert_err_re!(r, "^.* nonexistent masters: nonexistent1, nonexistent2$");

        // multiple config files in a specified directory
        let mut config = Config::new("pkgcraft", "");
        let t3 = EbuildRepoBuilder::new().name("r3").build().unwrap();
        let tmpdir = tempdir().unwrap();
        let conf_dir = tmpdir.path();
        let conf_path = conf_dir.to_str().unwrap();
        fs::create_dir(conf_dir.join("repos.conf")).unwrap();
        let data = indoc::formatdoc! {r#"
            [r1]
            location = {}
        "#, t1.path()};
        fs::write(conf_dir.join("repos.conf/r1.conf"), data).unwrap();
        let data = indoc::formatdoc! {r#"
            [r2]
            location = {}
            priority = -1
        "#, t2.path()};
        fs::write(conf_dir.join("repos.conf/r2.conf"), data).unwrap();
        let data = indoc::formatdoc! {r#"
            [r3]
            location = {}
            priority = 1
        "#, t3.path()};
        fs::write(conf_dir.join("repos.conf/r3.conf"), data).unwrap();
        config.load_portage_conf(Some(conf_path)).unwrap();
        config.finalize().unwrap();
        assert_ordered_eq!(config.repos().iter().map(|(_, r)| r.id()), ["r3", "r1", "r2"]);

        // reloading directory succeeds
        config.load_portage_conf(Some(conf_path)).unwrap();
        assert_ordered_eq!(config.repos().iter().map(|(_, r)| r.id()), ["r3", "r1", "r2"]);
    }

    #[test]
    fn add_repo() {
        let mut config = Config::new("pkgcraft", "");

        // nonexistent masters
        let path = test_data_path().join("repos/invalid/nonexistent-masters");
        let repo = Repo::from_path("test1", path, 0).unwrap();
        assert!(config.add_repo(repo).is_err());

        // empty
        let path = test_data_path().join("repos/valid/empty");
        let repo = Repo::from_path("test2", path, 0).unwrap();
        config.add_repo(repo).unwrap();

        // dynamically loaded masters
        let path = test_data_path().join("repos/valid/qa-secondary");
        let repo = Repo::from_path("test3", path, 0).unwrap();
        config.add_repo(repo).unwrap();

        // add directly via path
        let path = test_data_path().join("repos/valid/metadata");
        config.add_repo(&path).unwrap();
        config.add_repo(path.join("slot/slot")).unwrap();
        assert!(config.add_repo(path.join("nonexistent")).is_err());
    }
}
