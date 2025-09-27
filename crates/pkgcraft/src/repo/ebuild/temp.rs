use std::io::Write;
use std::{env, fs};

use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use tempfile::TempDir;

use crate::Error;
use crate::dep::Cpv;
use crate::eapi::{EAPI_LATEST_OFFICIAL, Eapi};
use crate::files::atomic_write_file;
use crate::pkg::ebuild::metadata::Key;
use crate::repo::ebuild::Metadata;
use crate::repo::{Repo, RepoFormat};

/// Temporary ebuild repo builder.
#[derive(Debug, Default)]
pub struct EbuildRepoBuilder {
    name: String,
    path: Option<Utf8PathBuf>,
    priority: i32,
    eapi: Option<&'static Eapi>,
}

impl EbuildRepoBuilder {
    /// Create the builder.
    pub fn new() -> Self {
        Self {
            name: "test".to_string(),
            ..Default::default()
        }
    }

    /// Set the repo name.
    pub fn name(mut self, value: &str) -> Self {
        self.name = value.to_string();
        self
    }

    /// Set the repo path.
    pub fn path<P: AsRef<Utf8Path>>(mut self, value: P) -> Self {
        self.path = Some(value.as_ref().to_path_buf());
        self
    }

    /// Set the repo priority.
    pub fn priority(mut self, value: i32) -> Self {
        self.priority = value;
        self
    }

    /// Set the repo EAPI.
    pub fn eapi(mut self, value: &'static Eapi) -> Self {
        self.eapi = Some(value);
        self
    }

    /// Build the temporary ebuild repo.
    pub fn build(self) -> crate::Result<EbuildTempRepo> {
        EbuildTempRepo::new(self.name, self.path.as_deref(), self.priority, self.eapi)
    }
}

/// A temporary repo that is automatically deleted when it goes out of scope.
#[derive(Debug)]
pub struct EbuildTempRepo {
    tempdir: TempDir,
    path: Utf8PathBuf,
    name: String,
    priority: i32,
}

impl EbuildTempRepo {
    /// Create a temporary repo at a path or inside `env::temp_dir()`.
    fn new(
        name: String,
        path: Option<&Utf8Path>,
        priority: i32,
        eapi: Option<&Eapi>,
    ) -> crate::Result<Self> {
        let path = match path {
            Some(p) => p.to_path_buf().into_std_path_buf(),
            None => env::temp_dir(),
        };
        let tempdir = TempDir::new_in(path)
            .map_err(|e| Error::RepoInit(format!("failed creating repo: {name}: {e}")))?;
        let path = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
            .map_err(|p| Error::RepoInit(format!("non-unicode temp path: {p:?}")))?;

        for dir in ["metadata", "profiles"] {
            fs::create_dir(path.join(dir))
                .map_err(|e| Error::RepoInit(format!("failed creating repo: {name}: {e}")))?;
        }

        let config = indoc::indoc! {"
            manifest-hashes = BLAKE2B SHA512
            manifest-required-hashes = BLAKE2B
            thin-manifests = true
        "};
        fs::write(path.join("metadata/layout.conf"), config)
            .map_err(|e| Error::RepoInit(format!("failed writing repo config: {e}")))?;

        fs::write(path.join("profiles/repo_name"), format!("{name}\n"))
            .map_err(|e| Error::RepoInit(format!("failed writing repo name: {e}")))?;

        let eapi = eapi.unwrap_or(&EAPI_LATEST_OFFICIAL);
        fs::write(path.join("profiles/eapi"), format!("{eapi}\n"))
            .map_err(|e| Error::RepoInit(format!("failed writing repo EAPI: {e}")))?;

        Ok(Self { tempdir, path, name, priority })
    }

    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Add a category into an ebuild repo's profiles/categories file.
    fn add_category(&mut self, category: &str) -> crate::Result<()> {
        let meta = Metadata::try_new(&self.name, &self.path)?;
        let mut categories = meta.categories().clone();
        if categories.insert(category.to_string()) {
            categories.sort_unstable();
            let data = categories.iter().map(|value| format!("{value}\n")).join("");
            atomic_write_file(self.path.join("profiles/categories"), data)?;
        }
        Ok(())
    }

    /// Create an ebuild using custom data field values.
    pub fn create_ebuild<T>(&mut self, value: T, data: &[&str]) -> crate::Result<Utf8PathBuf>
    where
        T: TryInto<Cpv>,
        Error: From<T::Error>,
    {
        let cpv = value.try_into()?;
        let path = self.path.join(format!("{}/{}.ebuild", cpv.cpn(), cpv.pf()));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating {cpv} dir: {e}")))?;
        let mut f = fs::File::create(&path)
            .map_err(|e| Error::IO(format!("failed creating {cpv} ebuild: {e}")))?;

        // ebuild defaults
        let mut values = indexmap::IndexMap::from([
            (Key::EAPI, EAPI_LATEST_OFFICIAL.as_str()),
            (Key::DESCRIPTION, "stub package description"),
            (Key::SLOT, "0"),
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

        self.add_category(cpv.category())?;
        Ok(path)
    }

    /// Create an ebuild file in the repo from raw data.
    pub fn create_ebuild_from_str<T>(
        &mut self,
        value: T,
        data: &str,
    ) -> crate::Result<Utf8PathBuf>
    where
        T: TryInto<Cpv>,
        Error: From<T::Error>,
    {
        let cpv = value.try_into()?;
        let path = self.path.join(format!("{}/{}.ebuild", cpv.cpn(), cpv.pf()));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating {cpv} dir: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| Error::IO(format!("failed writing to {cpv} ebuild: {e}")))?;
        self.add_category(cpv.category())?;
        Ok(path)
    }

    /// Create an eclass in the repo.
    pub fn create_eclass(&mut self, name: &str, data: &str) -> crate::Result<Utf8PathBuf> {
        let path = self.path.join(format!("eclass/{name}.eclass"));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating eclass dir: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| Error::IO(format!("failed writing to eclass: {e}")))?;
        Ok(path)
    }

    /// Persist the temporary repo to disk, returning the [`Utf8PathBuf`] where it is located.
    pub fn persist<P: AsRef<Utf8Path>>(self, path: Option<P>) -> crate::Result<Utf8PathBuf> {
        let mut repo_path = Utf8PathBuf::from_path_buf(self.tempdir.keep())
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

impl From<&EbuildTempRepo> for Repo {
    fn from(repo: &EbuildTempRepo) -> Self {
        RepoFormat::Ebuild
            .from_path(&repo.name, &repo.path, repo.priority)
            .unwrap()
            .into_ebuild()
            .unwrap()
            .into()
    }
}

impl AsRef<std::path::Path> for EbuildTempRepo {
    fn as_ref(&self) -> &std::path::Path {
        self.path().as_ref()
    }
}

impl AsRef<std::ffi::OsStr> for EbuildTempRepo {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.path().as_ref()
    }
}
