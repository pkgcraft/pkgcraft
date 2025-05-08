use std::fs;
use std::io::{self, Write};
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use tempfile::TempDir;
use tracing::error;

use crate::Error;
use crate::repo::set::RepoSet;
use crate::repo::{Repo, RepoFormat, Repository};
use crate::sync::Syncer;

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct RepoConfig {
    pub(crate) location: Utf8PathBuf,
    #[serde_as(as = "DisplayFromStr")]
    pub(crate) format: RepoFormat,
    pub(crate) priority: Option<i32>,
    pub(crate) sync: Option<Syncer>,
}

impl From<RepoFormat> for RepoConfig {
    fn from(format: RepoFormat) -> Self {
        Self {
            location: Default::default(),
            format,
            priority: Default::default(),
            sync: Default::default(),
        }
    }
}

impl RepoConfig {
    fn from_path<P: AsRef<Utf8Path>>(path: P) -> crate::Result<Self> {
        let path = path.as_ref();
        let data = fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("failed loading repo config: {path}: {e}")))?;

        let config: RepoConfig = toml::from_str(&data).map_err(|e| {
            Error::Config(format!("failed loading repo config toml: {path}: {e}"))
        })?;

        Ok(config)
    }

    /// Try loading the repository from the config.
    fn to_repo(&self) -> crate::Result<Repo> {
        let name = self.location.file_name().unwrap_or_default();
        self.format.from_path(name, &self.location, self.priority())
    }

    /// Return the repository's configured priority.
    pub(crate) fn priority(&self) -> i32 {
        self.priority.unwrap_or_default()
    }

    /// Remove the repo files.
    fn remove(&self) -> crate::Result<()> {
        let path = &self.location;

        let result = match &self.sync {
            Some(Syncer::Local(_)) => fs::remove_file(path),
            _ => fs::remove_dir_all(path),
        };

        result.map_err(|e| Error::IO(format!("failed removing repo: {path}: {e}")))
    }

    /// Sync repository to its configured location.
    pub(crate) fn sync(&self) -> crate::Result<()> {
        match &self.sync {
            Some(syncer) => syncer.sync(&self.location),
            None => Ok(()),
        }
    }
}

/// Builder for adding repos to a configuration.
pub struct RepoConfigBuilder<'a> {
    name: Option<String>,
    inner: RepoConfig,
    config: &'a ConfigRepos,
    tmpdir: Option<TempDir>,
}

impl<'a> RepoConfigBuilder<'a> {
    fn new(config: &'a ConfigRepos, uri: &str) -> crate::Result<Self> {
        let syncer: Syncer = uri.parse()?;
        let (tmpdir, location) = if let Syncer::Local(repo) = &syncer {
            (None, config.repos_dir.join(&repo.name))
        } else {
            let dir = TempDir::new_in(&config.repos_dir)
                .map_err(|e| Error::InvalidValue(format!("failed creating temp dir: {e}")))?;
            let path = Utf8Path::from_path(dir.path())
                .ok_or_else(|| Error::InvalidValue("invalid temp dir path".to_string()))
                .map(|x| x.to_path_buf())?;
            (Some(dir), path)
        };

        let inner = RepoConfig {
            location,
            priority: Default::default(),
            sync: Some(syncer),
            ..RepoFormat::Ebuild.into()
        };

        Ok(Self {
            name: None,
            inner,
            config,
            tmpdir,
        })
    }

    /// Modify the repository name.
    pub fn name<S: std::fmt::Display>(&mut self, value: S) {
        self.name = Some(value.to_string());
    }

    /// Modify the repository priority.
    pub fn priority(&mut self, value: i32) {
        self.inner.priority = Some(value);
    }

    /// Add the repo to the config, optionally syncing.
    pub fn add_to_config(mut self, sync: bool) -> crate::Result<()> {
        // create repos directory
        let dir = &self.config.repos_dir;
        fs::create_dir_all(dir)
            .map_err(|e| Error::Config(format!("failed creating config dir: {dir}: {e}")))?;

        // optionally sync the repo
        let mut synced_repo = None;
        if sync {
            self.inner.sync()?;
            synced_repo = Some(self.inner.to_repo()?);
        }

        // determine the repo name
        let name = if let Some(value) = self.name {
            Ok(value)
        } else if let Some(repo) = synced_repo {
            Ok(repo.name().to_string())
        } else {
            Err(Error::InvalidValue("missing repo name".to_string()))
        }?;

        let repo_path = self.config.repos_dir.join(&name);
        self.inner.location = repo_path.clone();

        // persist the synced temporary repo to disk
        if sync {
            if let Some(dir) = self.tmpdir {
                let path = dir.into_path();
                if let Err(e) = fs::rename(&path, &repo_path) {
                    if e.kind() == io::ErrorKind::DirectoryNotEmpty {
                        return Err(Error::Config(format!("existing repo: {name}")));
                    } else {
                        return Err(Error::IO(format!(
                            "failed moving repo to: {repo_path}: {e}"
                        )));
                    }
                }
            }
        }

        // create config directory
        let dir = &self.config.config_dir;
        fs::create_dir_all(dir)
            .map_err(|e| Error::Config(format!("failed creating config dir: {dir}: {e}")))?;
        let config_path = self.config.config_dir.join(&name);
        if config_path.exists() {
            return Err(Error::Config(format!("existing repo config: {name}")));
        }

        // write repo config file to disk
        let data = toml::to_string(&self.inner)
            .map_err(|e| Error::Config(format!("failed serializing repo config: {e}")))?;
        let mut file = fs::File::create(&config_path).map_err(|e| {
            Error::Config(format!("failed creating repo config file: {config_path}: {e}"))
        })?;
        file.write_all(data.as_bytes()).map_err(|e| {
            Error::Config(format!("failed writing repo config file: {config_path}: {e}"))
        })?;

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct ConfigRepos {
    config_dir: Utf8PathBuf,
    repos_dir: Utf8PathBuf,
    repos: IndexMap<String, Repo>,
    configured: IndexSet<Repo>,
    nonexistent: IndexMap<String, RepoConfig>,
}

impl ConfigRepos {
    pub(super) fn new(
        config_dir: &Utf8Path,
        db_dir: &Utf8Path,
        settings: &Arc<super::Settings>,
    ) -> crate::Result<Self> {
        let config_dir = config_dir.join("repos");
        let repos_dir = db_dir.join("repos");

        let mut configs = vec![];
        if config_dir.exists() {
            let entries = config_dir
                .read_dir_utf8()
                .map_err(|e| Error::Config(e.to_string()))?;

            for entry in entries {
                let entry = entry.map_err(|e| Error::Config(e.to_string()))?;
                if entry.file_type().map(|x| x.is_file()).unwrap_or_default()
                    && !entry.file_name().starts_with('.')
                {
                    // ignore bad configs
                    match RepoConfig::from_path(entry.path()) {
                        Ok(config) => {
                            configs.push((entry.file_name().to_string(), config));
                        }
                        Err(err) => error!("{err}"),
                    }
                }
            }
        }

        // load repos
        let mut repos = vec![];
        let mut nonexistent = IndexMap::new();
        for (name, config) in configs {
            // ignore invalid repos
            match Repo::from_config(&name, &config) {
                Ok(repo) => repos.push(repo),
                Err(Error::NonexistentRepo(_)) => {
                    nonexistent.insert(name, config);
                }
                Err(err) => error!("{err}"),
            }
        }

        nonexistent.sort_keys();

        let mut config = Self {
            config_dir,
            repos_dir,
            nonexistent,
            ..Default::default()
        };

        // add repos to the config
        config.extend(repos, settings)?;
        Ok(config)
    }

    /// Create a repo from a URI.
    pub fn add_uri(&self, uri: &str) -> crate::Result<RepoConfigBuilder> {
        RepoConfigBuilder::new(self, uri)
    }

    /// Remove repos from the config.
    pub fn remove<I>(&mut self, repos: I) -> crate::Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut nonexistent = vec![];

        for name in repos {
            let name = name.as_ref();
            // error out if repo config is missing
            // physical repo files are allowed to be missing
            if let Some(repo) = self.repos.shift_remove(name) {
                repo.repo_config().remove()?;
                let path = self.config_dir.join(name);
                fs::remove_file(&path).map_err(|e| {
                    Error::IO(format!("failed removing repo config: {path}: {e}"))
                })?;
            } else {
                nonexistent.push(name.to_string());
            }
        }

        if !nonexistent.is_empty() {
            let nonexistent = nonexistent.iter().join(", ");
            Err(Error::InvalidValue(format!(
                "failed removing nonexistent repos: {nonexistent}"
            )))
        } else {
            Ok(())
        }
    }

    // TODO: add concurrent syncing support with output progress
    pub fn sync<I>(&self, values: I) -> crate::Result<()>
    where
        I: IntoIterator,
        I::Item: std::fmt::Display,
    {
        let mut repos = vec![];
        for id in values {
            let id = id.to_string();
            if let Some(repo) = self.repos.get(&id) {
                repos.push((id, repo.repo_config()));
            } else if let Some(config) = self.nonexistent.get(&id) {
                repos.push((id, config));
            } else {
                return Err(Error::NonexistentRepo(id));
            }
        }

        // sync all repos if none were passed
        if repos.is_empty() {
            repos.extend(
                self.repos
                    .iter()
                    .map(|(id, r)| (id.clone(), r.repo_config())),
            );
            repos.extend(self.nonexistent.iter().map(|(id, c)| (id.clone(), c)));
        }

        let mut failed = vec![];
        for (name, repo) in repos {
            if let Err(e) = repo.sync() {
                failed.push((name, e));
            }
        }

        if failed.is_empty() {
            Ok(())
        } else {
            let errors = failed
                .iter()
                .map(|(name, e)| format!("{name}: {e}"))
                .join("\n\t");
            Err(Error::Config(format!("failed syncing:\n\t{errors}")))
        }
    }

    /// Return true if no repos exist.
    pub fn is_empty(&self) -> bool {
        self.repos.is_empty()
    }

    /// RepoSet objects from sets of repos registered in the config object.
    pub fn set(&self, kind: Option<RepoFormat>) -> RepoSet {
        let repos = self.repos.values();
        match kind {
            None => repos.collect(),
            Some(RepoFormat::Ebuild) => repos.filter(|r| r.is_ebuild()).collect(),
            Some(RepoFormat::Configured) => self.configured.iter().collect(),
            Some(RepoFormat::Fake) => repos.filter(|r| r.is_fake()).collect(),
        }
    }

    /// Get a repo.
    pub(crate) fn get<S: AsRef<str>>(&self, key: S) -> crate::Result<&Repo> {
        let key = key.as_ref();
        self.repos
            .get(key)
            .ok_or_else(|| Error::InvalidValue(format!("nonexistent repo: {key}")))
    }

    /// Extend the config with multiple repos.
    pub(crate) fn extend<I: IntoIterator<Item = Repo>>(
        &mut self,
        repos: I,
        settings: &Arc<super::Settings>,
    ) -> crate::Result<()> {
        let mut existing_repos = vec![];
        let mut new_repos = IndexMap::new();

        // determine if any new repos override existing ones
        for repo in repos {
            if let Some(existing) = self.repos.get(repo.id()) {
                if existing != &repo {
                    existing_repos.push(repo);
                }
            } else {
                new_repos.insert(repo.id().to_string(), repo);
            }
        }

        // error out on overriding repos
        if !existing_repos.is_empty() {
            let repos = existing_repos.iter().map(|r| r.id()).join(", ");
            return Err(Error::Config(format!("can't override existing repos: {repos}")));
        }

        for (_name, repo) in &new_repos {
            // create configured ebuild repos
            if let Repo::Ebuild(r) = repo {
                let configured = r.configure(settings.clone());
                self.configured.insert(configured.into());
            }
        }

        // add new repos to config
        self.repos.extend(new_repos);

        // sort raw and configured repos
        self.repos.sort_unstable_by(|_, r1, _, r2| r1.cmp(r2));
        self.configured.sort_unstable();

        Ok(())
    }

    pub fn iter(&self) -> ReposIter<'_> {
        self.into_iter()
    }
}

pub struct ReposIter<'a> {
    iter: indexmap::map::Iter<'a, String, Repo>,
}

impl<'a> IntoIterator for &'a ConfigRepos {
    type Item = (&'a str, &'a Repo);
    type IntoIter = ReposIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ReposIter { iter: self.repos.iter() }
    }
}

impl<'a> Iterator for ReposIter<'a> {
    type Item = (&'a str, &'a Repo);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(id, repo)| (id.as_str(), repo))
    }
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use crate::config::Config;
    use crate::repo::FakeRepo;
    use crate::test::*;

    use super::*;

    #[test]
    fn repo_config() {
        // nonexistent
        assert!(RepoConfig::from_path("nonexistent").is_err());

        // empty
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let r = RepoConfig::from_path(path);
        assert_err_re!(r, "missing field `location`");

        // invalid (missing format)
        let data = indoc::indoc! {r#"
            location = "/path/to/repo"
        "#};
        fs::write(&file, data).unwrap();
        let r = RepoConfig::from_path(path);
        assert_err_re!(r, "missing field `format`");

        // invalid (invalid syncer)
        let data = indoc::indoc! {r#"
            location = "/path/to/repo"
            sync = "invalid"
        "#};
        fs::write(&file, data).unwrap();
        let r = RepoConfig::from_path(path);
        assert_err_re!(r, "no syncers available: invalid");

        // valid (all required fields)
        let data = indoc::indoc! {r#"
            location = "/path/to/repo"
            format = "ebuild"
        "#};
        fs::write(&file, data).unwrap();
        RepoConfig::from_path(path).unwrap();

        // valid (all fields)
        let data = indoc::indoc! {r#"
            location = "/path/to/repo"
            format = "ebuild"
            priority = 0
            sync = "tar+https://pkgcraft.pkgcraft/repo.tar.gz"
        "#};
        fs::write(&file, data).unwrap();
        RepoConfig::from_path(path).unwrap();
    }

    #[test]
    fn sync() {
        let mut config = Config::new("pkgcraft", "");

        // nonexistent repo
        let r = config.repos().sync(["nonexistent"]);
        assert_err_re!(r, "nonexistent repo: nonexistent");

        // fake repo with no-op syncing
        let fake_repo = FakeRepo::new("fake", 0).pkgs(["cat/pkg-1"]).unwrap();
        config.add_repo(fake_repo).unwrap();
        assert!(config.repos().sync(["fake"]).is_ok());

        // all repos
        let repos: [&str; 0] = [];
        assert!(config.repos().sync(repos).is_ok());
    }
}
