use std::io::Write;
use std::{env, fs};

use camino::{Utf8Path, Utf8PathBuf};
use tempfile::TempDir;

use crate::atom::Atom;
use crate::{eapi, Error};

/// A temporary repo that is automatically deleted when it goes out of scope.
#[derive(Debug)]
pub struct Repo {
    tempdir: TempDir,
    path: Utf8PathBuf,
}

impl Repo {
    /// Create a temporary repo at a given path or inside `env::temp_dir()`.
    pub fn new(
        id: &str,
        path: Option<&Utf8Path>,
        eapi: Option<&eapi::Eapi>,
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
        fs::write(temp_path.join("profiles/repo_name"), format!("{id}\n"))
            .map_err(|e| Error::RepoInit(format!("failed writing repo id: {e}")))?;

        if let Some(eapi) = eapi {
            fs::write(temp_path.join("profiles/eapi"), format!("{eapi}\n"))
                .map_err(|e| Error::RepoInit(format!("failed writing repo EAPI: {e}")))?;
        }

        let path = Utf8PathBuf::from_path_buf(temp_path.to_path_buf())
            .map_err(|p| Error::RepoInit(format!("non-unicode repo path: {p:?}")))?;
        Ok(Self { tempdir, path })
    }

    /// Create an ebuild file in the repo.
    pub fn create_ebuild<'a, I>(&self, cpv: &str, data: I) -> crate::Result<(Utf8PathBuf, Atom)>
    where
        I: IntoIterator<Item = (crate::pkgsh::metadata::Key, &'a str)>,
    {
        use crate::pkgsh::metadata::Key::*;
        let cpv = Atom::new_cpv(cpv)?;
        let path = self.path.join(format!(
            "{}/{}-{}.ebuild",
            cpv.cpn(),
            cpv.package(),
            cpv.version().unwrap()
        ));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating {cpv} dir: {e}")))?;
        let mut f = fs::File::create(&path)
            .map_err(|e| Error::IO(format!("failed creating {cpv} ebuild: {e}")))?;

        // ebuild defaults
        let mut values = indexmap::IndexMap::from([
            (Eapi, eapi::EAPI_LATEST.as_str()),
            (Slot, "0"),
            (Description, "stub package description"),
            (Homepage, "https://github.com/pkgcraft"),
        ]);

        // overrides defaults with specified values, removing the defaults for "-"
        for (key, val) in data.into_iter() {
            match val {
                "-" => values.remove(&key),
                _ => values.insert(key, val),
            };
        }

        for (key, val) in values {
            f.write(format!("{key}=\"{val}\"\n").as_bytes())
                .map_err(|e| Error::IO(format!("failed writing to {cpv} ebuild: {e}")))?;
        }

        Ok((path, cpv))
    }

    /// Create an ebuild file in the repo from raw data.
    pub fn create_ebuild_raw(&self, cpv: &str, data: &str) -> crate::Result<(Utf8PathBuf, Atom)> {
        let cpv = Atom::new_cpv(cpv)?;
        let path = self.path.join(format!(
            "{}/{}-{}.ebuild",
            cpv.cpn(),
            cpv.package(),
            cpv.version().unwrap()
        ));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating {cpv} dir: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| Error::IO(format!("failed writing to {cpv} ebuild: {e}")))?;
        Ok((path, cpv))
    }

    /// Create an eclass in the repo.
    pub fn create_eclass(&self, name: &str, data: &str) -> crate::Result<Utf8PathBuf> {
        let path = self.path.join(format!("eclass/{name}.eclass"));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating eclass dir: {e}")))?;
        fs::write(&path, data).map_err(|e| Error::IO(format!("failed writing to eclass: {e}")))?;
        Ok(path)
    }

    /// Return the temporary repo's file path.
    pub fn path(&self) -> &Utf8Path {
        &self.path
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
