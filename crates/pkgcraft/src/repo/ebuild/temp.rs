use std::io::Write;
use std::ops::Deref;
use std::sync::{Arc, OnceLock};
use std::{env, fs};

use camino::{Utf8Path, Utf8PathBuf};
use tempfile::TempDir;

use crate::dep::Cpv;
use crate::eapi::{Eapi, EAPI_LATEST_OFFICIAL};
use crate::pkg::ebuild::{self, metadata::Key};
use crate::repo::Repo as BaseRepo;
use crate::Error;

/// A temporary repo that is automatically deleted when it goes out of scope.
#[derive(Debug)]
pub struct Repo {
    tempdir: TempDir,
    path: Utf8PathBuf,
    id: String,
    priority: i32,
    repo: OnceLock<Arc<super::Repo>>,
}

impl Repo {
    /// Create a temporary repo at a given path or inside `env::temp_dir()`.
    pub fn new(
        id: &str,
        path: Option<&Utf8Path>,
        priority: i32,
        eapi: Option<&Eapi>,
    ) -> crate::Result<Self> {
        let path = match path {
            Some(p) => p.to_path_buf().into_std_path_buf(),
            None => env::temp_dir(),
        };
        let tempdir = TempDir::new_in(path)
            .map_err(|e| Error::RepoInit(format!("failed creating repo {id:?}: {e}")))?;
        let temp_path = tempdir.path();

        for dir in ["metadata", "profiles"] {
            fs::create_dir(temp_path.join(dir))
                .map_err(|e| Error::RepoInit(format!("failed creating repo {id:?}: {e}")))?;
        }

        let config = indoc::indoc! {"
            manifest-hashes = BLAKE2B SHA512
            manifest-required-hashes = BLAKE2B
        "};
        fs::write(temp_path.join("metadata/layout.conf"), config)
            .map_err(|e| Error::RepoInit(format!("failed writing repo id: {e}")))?;

        fs::write(temp_path.join("profiles/repo_name"), format!("{id}\n"))
            .map_err(|e| Error::RepoInit(format!("failed writing repo id: {e}")))?;

        let eapi = eapi.unwrap_or(&EAPI_LATEST_OFFICIAL);
        fs::write(temp_path.join("profiles/eapi"), format!("{eapi}\n"))
            .map_err(|e| Error::RepoInit(format!("failed writing repo EAPI: {e}")))?;

        let path = Utf8PathBuf::from_path_buf(temp_path.to_path_buf())
            .map_err(|p| Error::RepoInit(format!("non-unicode repo path: {p:?}")))?;

        Ok(Self {
            tempdir,
            path,
            id: id.to_string(),
            priority,
            repo: OnceLock::new(),
        })
    }

    fn repo(&self) -> &Arc<super::Repo> {
        self.repo.get_or_init(|| {
            let repo = super::Repo::from_path(&self.id, self.priority, &self.path)
                .unwrap_or_else(|e| panic!("failed creating temporary ebuild repo: {e}"));
            Arc::new(repo)
        })
    }

    /// Create a [`ebuild::raw::Pkg`] from ebuild field settings.
    pub fn create_raw_pkg<S: AsRef<str>>(
        &self,
        cpv: S,
        data: &[&str],
    ) -> crate::Result<ebuild::raw::Pkg> {
        use Key::*;
        let cpv = Cpv::try_new(cpv.as_ref())?;
        let path = self.path.join(format!("{}/{}.ebuild", cpv.cpn(), cpv.pf()));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating {cpv} dir: {e}")))?;
        let mut f = fs::File::create(&path)
            .map_err(|e| Error::IO(format!("failed creating {cpv} ebuild: {e}")))?;

        // ebuild defaults
        let mut values = indexmap::IndexMap::from([
            (EAPI, EAPI_LATEST_OFFICIAL.as_str()),
            (SLOT, "0"),
            (DESCRIPTION, "stub package description"),
        ]);

        // overrides defaults with specified values, removing the defaults for "-"
        for s in data {
            let (key, val) = s.split_once('=').unwrap_or((s, ""));
            let key = key
                .parse()
                .map_err(|_| Error::InvalidValue(format!("invalid metadata key: {key}")))?;
            match val {
                "" => values.swap_remove(&key),
                _ => values.insert(key, val),
            };
        }

        for (key, val) in values {
            f.write(format!("{key}=\"{val}\"\n").as_bytes())
                .map_err(|e| Error::IO(format!("failed writing to {cpv} ebuild: {e}")))?;
        }

        ebuild::raw::Pkg::try_new(cpv, self)
    }

    /// Create a [`ebuild::Pkg`] from ebuild field settings.
    pub fn create_pkg<S: AsRef<str>>(&self, cpv: S, data: &[&str]) -> crate::Result<ebuild::Pkg> {
        let raw_pkg = self.create_raw_pkg(cpv, data)?;
        raw_pkg.try_into()
    }

    /// Create an ebuild file in the repo from raw data.
    pub fn create_raw_pkg_from_str<S: AsRef<str>>(
        &self,
        cpv: S,
        data: &str,
    ) -> crate::Result<ebuild::raw::Pkg> {
        let cpv = Cpv::try_new(cpv)?;
        let path = self.path.join(format!("{}/{}.ebuild", cpv.cpn(), cpv.pf()));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating {cpv} dir: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| Error::IO(format!("failed writing to {cpv} ebuild: {e}")))?;
        ebuild::raw::Pkg::try_new(cpv, self)
    }

    /// Create a [`ebuild::Pkg`] from an ebuild using raw data.
    pub fn create_pkg_from_str<S: AsRef<str>>(
        &self,
        cpv: S,
        data: &str,
    ) -> crate::Result<ebuild::Pkg> {
        let raw_pkg = self.create_raw_pkg_from_str(cpv, data)?;
        raw_pkg.try_into()
    }

    /// Create an eclass in the repo.
    pub fn create_eclass(&self, name: &str, data: &str) -> crate::Result<Utf8PathBuf> {
        let path = self.path.join(format!("eclass/{name}.eclass"));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating eclass dir: {e}")))?;
        fs::write(&path, data).map_err(|e| Error::IO(format!("failed writing to eclass: {e}")))?;
        Ok(path)
    }

    /// Persist the temporary repo to disk, returning the [`Utf8PathBuf`] where it is located.
    pub fn persist<P: AsRef<Utf8Path>>(self, path: Option<P>) -> crate::Result<Utf8PathBuf> {
        let mut repo_path = Utf8PathBuf::from_path_buf(self.tempdir.into_path())
            .map_err(|p| Error::IO(format!("non-unicode repo path: {p:?}")))?;
        if let Some(path) = path {
            let path = path.as_ref();
            fs::rename(&repo_path, path).map_err(|e| {
                Error::IO(format!("failed renaming repo: {repo_path:?} -> {path:?}: {e}"))
            })?;
            repo_path = path.to_path_buf();
        }
        Ok(repo_path)
    }
}

impl Deref for Repo {
    type Target = super::Repo;

    fn deref(&self) -> &Self::Target {
        self.repo()
    }
}

impl From<&Repo> for BaseRepo {
    fn from(value: &Repo) -> Self {
        BaseRepo::Ebuild(value.repo().clone())
    }
}
