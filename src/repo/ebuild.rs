use std::collections::HashSet;
#[cfg(test)]
use std::io::Write;
use std::iter;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, fmt, fs, io};

use ini::Ini;
use itertools::Either;
use once_cell::sync::Lazy;
use tempfile::TempDir;
use tracing::warn;
use walkdir::{DirEntry, WalkDir};

use crate::config::Config;
use crate::macros::build_from_paths;
use crate::pkg::Pkg;
use crate::types::WalkDirFilter;
use crate::{atom, eapi, repo, Error, Result};

const DEFAULT_SECTION: Option<String> = None;
static FAKE_CATEGORIES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["eclass", "profiles", "metadata", "licenses"]
        .iter()
        .cloned()
        .collect()
});

pub(crate) struct Metadata {
    path: Option<PathBuf>,
    ini: Ini,
}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let section = self.ini.section(DEFAULT_SECTION);
        f.debug_struct("Metadata")
            .field("path", &self.path)
            .field("ini", &section)
            .finish()
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata {
            path: None,
            ini: Ini::new(),
        }
    }
}

impl Metadata {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        match Ini::load_from_file(path) {
            Ok(ini) => Ok(Metadata {
                path: Some(PathBuf::from(path)),
                ini,
            }),
            Err(ini::Error::Io(e)) if e.kind() == io::ErrorKind::NotFound => Ok(Metadata {
                path: Some(PathBuf::from(path)),
                ini: Ini::new(),
            }),
            Err(e) => Err(Error::InvalidValue(format!("invalid repo layout: {path:?}: {e}"))),
        }
    }

    #[cfg(test)]
    fn set<S1, S2>(&mut self, key: S1, val: S2)
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        self.ini.set_to(DEFAULT_SECTION, key.into(), val.into());
    }

    #[cfg(test)]
    fn write(&self, data: Option<&str>) -> Result<()> {
        if let Some(path) = &self.path {
            self.ini
                .write_to_file(path)
                .map_err(|e| Error::IO(e.to_string()))?;

            if let Some(data) = data {
                let mut f = fs::File::options()
                    .append(true)
                    .open(path)
                    .map_err(|e| Error::IO(e.to_string()))?;
                write!(f, "{}", data).map_err(|e| Error::IO(e.to_string()))?;
            }
        }

        Ok(())
    }

    fn get_list<S: AsRef<str>>(&self, key: S) -> Vec<String> {
        match self.ini.get_from(DEFAULT_SECTION, key.as_ref()) {
            None => vec![],
            Some(s) => s.split_whitespace().map(|s| s.into()).collect(),
        }
    }

    pub(crate) fn masters(&self) -> Vec<String> {
        self.get_list("masters")
    }
}

#[derive(Debug, Default)]
pub struct Repo {
    id: String,
    pub(super) path: PathBuf,
    pub(super) config: Metadata,
}

impl Repo {
    pub(super) const FORMAT: &'static str = "ebuild";

    fn new<S, P>(id: S, path: P, config: Metadata) -> Result<Self>
    where
        S: AsRef<str>,
        P: AsRef<Path>,
    {
        Ok(Repo {
            id: id.as_ref().to_string(),
            path: PathBuf::from(path.as_ref()),
            config,
        })
    }

    pub(super) fn from_path<S, P>(id: S, path: P) -> Result<Self>
    where
        S: AsRef<str>,
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let profiles_base = path.join("profiles");

        if !profiles_base.exists() {
            return Err(Error::InvalidRepo {
                path: PathBuf::from(path),
                error: "missing profiles dir".to_string(),
            });
        }

        let config =
            Metadata::new(path.join("metadata/layout.conf")).map_err(|e| Error::InvalidRepo {
                path: PathBuf::from(path),
                error: e.to_string(),
            })?;
        Repo::new(id, path, config)
    }

    pub fn masters(&self) -> Result<Vec<Arc<repo::Repository>>> {
        let config = Config::current();
        let mut masters = vec![];
        for id in self.config.masters() {
            match config.repos.repos.get(&id) {
                Some(r) => masters.push(r.clone()),
                None => {
                    return Err(Error::InvalidRepo {
                        path: self.path.clone(),
                        error: format!("nonexistent master: {id}"),
                    })
                }
            }
        }
        Ok(masters)
    }

    pub fn trees(&self) -> Result<Vec<Arc<repo::Repository>>> {
        let mut trees = self.masters()?;
        let config = Config::current();
        match config.repos.repos.get(&self.id) {
            Some(r) => trees.push(r.clone()),
            None => {
                return Err(Error::InvalidRepo {
                    path: self.path.clone(),
                    error: format!("unconfigured repo: {}", self.id),
                })
            }
        }
        Ok(trees)
    }

    pub fn category_dirs(&self) -> Vec<String> {
        // filter out non-category dirs
        let filter = |e: &DirEntry| -> bool { is_dir(e) && !is_hidden(e) && !is_fake_category(e) };
        let cats = FilesAtPath::new(&self.path, Some(filter));
        let mut v = vec![];
        for entry in cats {
            let path = entry.path();
            match entry.file_name().to_str() {
                Some(s) => match atom::parse::category(s) {
                    Ok(cat) => v.push(cat.into()),
                    Err(e) => warn!("{e}: {path:?}"),
                },
                None => warn!("non-unicode path: {path:?}"),
            }
        }
        v
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.id, self.path.to_string_lossy())
    }
}

struct FilesAtPath(Box<dyn Iterator<Item = walkdir::Result<DirEntry>>>);

impl FilesAtPath {
    fn new<P>(path: P, predicate: Option<WalkDirFilter>) -> Self
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let entries = WalkDir::new(path)
            .sort_by_file_name()
            .min_depth(1)
            .max_depth(1);

        // optionally apply directory filtering
        let entries = match predicate.as_ref().cloned() {
            None => Either::Left(entries),
            Some(func) => Either::Right(entries.into_iter().filter_entry(func)),
        };

        FilesAtPath(Box::new(entries.into_iter()))
    }
}

impl Iterator for FilesAtPath {
    type Item = DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some(Ok(entry)) => Some(entry),
            _ => None,
        }
    }
}

fn is_dir(entry: &DirEntry) -> bool {
    entry.path().is_dir()
}

fn is_file(entry: &DirEntry) -> bool {
    entry.path().is_file()
}

fn has_ext(entry: &DirEntry, ext: &str) -> bool {
    match entry.path().extension() {
        Some(e) => e.to_str() == Some(ext),
        _ => false,
    }
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn is_fake_category(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| FAKE_CATEGORIES.contains(s))
        .unwrap_or(false)
}

impl repo::Repo for Repo {
    fn categories(&self) -> Vec<String> {
        // TODO: implement reading profiles/categories, falling back to category_dirs()
        self.category_dirs()
    }

    fn packages(&self, cat: &str) -> Vec<String> {
        let path = self.path.join(cat.strip_prefix('/').unwrap_or(cat));
        let filter = |e: &DirEntry| -> bool { is_dir(e) && !is_hidden(e) };
        let pkgs = FilesAtPath::new(&path, Some(filter));
        let mut v = vec![];
        for entry in pkgs {
            let path = entry.path();
            match entry.file_name().to_str() {
                Some(s) => match atom::parse::package(s) {
                    Ok(pn) => v.push(pn.into()),
                    Err(e) => warn!("{e}: {path:?}"),
                },
                None => warn!("non-unicode path: {path:?}"),
            }
        }
        v
    }

    fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
        let path = build_from_paths!(
            &self.path,
            cat.strip_prefix('/').unwrap_or(cat),
            pkg.strip_prefix('/').unwrap_or(pkg)
        );
        let filter = |e: &DirEntry| -> bool { is_file(e) && !is_hidden(e) && has_ext(e, "ebuild") };
        let ebuilds = FilesAtPath::new(&path, Some(filter));
        let mut v = vec![];
        for entry in ebuilds {
            let path = entry.path();
            let pn = path.parent().unwrap().file_name().unwrap().to_str();
            let pf = path.file_stem().unwrap().to_str();
            match (pn, pf) {
                (Some(pn), Some(pf)) => match pn == &pf[..pn.len()] {
                    true => match atom::parse::version(&pf[pn.len() + 1..]) {
                        Ok(ver) => v.push(format!("{}", ver)),
                        Err(e) => warn!("{e}: {path:?}"),
                    },
                    false => warn!("unmatched ebuild: {path:?}"),
                },
                _ => warn!("non-unicode path: {path:?}"),
            }
        }
        v
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn iter(&self) -> Box<dyn Iterator<Item = Box<dyn Pkg>>> {
        Box::new(iter::empty::<Box<dyn Pkg>>())
    }
}

/// A temporary repo that is automatically deleted when it goes out of scope.
#[derive(Debug)]
pub(crate) struct TempRepo {
    tempdir: TempDir,
    repo: Repo,
}

impl TempRepo {
    /// Attempts to create a temporary repo inside an optional path or inside `env::temp_dir()` if
    /// no path is specified.
    pub(crate) fn new<P: AsRef<Path>>(
        id: &str,
        path: Option<P>,
        eapi: Option<&eapi::Eapi>,
    ) -> Result<Self> {
        let path = match path {
            Some(p) => PathBuf::from(p.as_ref()),
            None => env::temp_dir(),
        };
        let eapi = format!("{}", eapi.unwrap_or(eapi::EAPI_LATEST));
        let tempdir = TempDir::new_in(path)
            .map_err(|e| Error::RepoInit(format!("failed creating temp repo {id:?}: {e}")))?;
        let temp_path = tempdir.path();

        for dir in ["metadata", "profiles"] {
            fs::create_dir(temp_path.join(dir))
                .map_err(|e| Error::RepoInit(format!("failed creating temp repo {id:?}: {e}")))?;
        }
        fs::write(temp_path.join("profiles/repo_name"), format!("{id}\n"))
            .map_err(|e| Error::RepoInit(format!("failed writing temp repo id: {e}")))?;
        fs::write(temp_path.join("profiles/eapi"), format!("{eapi}\n"))
            .map_err(|e| Error::RepoInit(format!("failed writing temp repo EAPI: {e}")))?;

        let repo = Repo::from_path(id, temp_path)?;
        Ok(TempRepo { tempdir, repo })
    }

    /// Attempts to persist the temporary repo to disk, returning the [`PathBuf`] where it is
    /// located.
    pub(crate) fn persist<P: AsRef<Path>>(self, path: Option<P>) -> Result<PathBuf> {
        let mut repo_path = self.tempdir.into_path();
        if let Some(path) = path {
            let path = path.as_ref();
            fs::rename(&repo_path, path).map_err(|e| {
                Error::RepoInit(format!("failed renaming repo: {repo_path:?} -> {path:?}: {e}"))
            })?;
            repo_path = path.into();
        }
        Ok(repo_path)
    }
}

impl fmt::Display for TempRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.repo)
    }
}

impl repo::Repo for TempRepo {
    #[inline]
    fn categories(&self) -> Vec<String> {
        self.repo.categories()
    }

    #[inline]
    fn packages(&self, cat: &str) -> Vec<String> {
        self.repo.packages(cat)
    }

    #[inline]
    fn versions(&self, cat: &str, pkg: &str) -> Vec<String> {
        self.repo.versions(cat, pkg)
    }

    #[inline]
    fn id(&self) -> &str {
        &self.repo.id
    }

    #[inline]
    fn iter(&self) -> Box<dyn Iterator<Item = Box<dyn Pkg>>> {
        self.repo.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::repo::Repo as RepoTrait;

    use super::*;

    #[test]
    fn test_masters() {
        let temprepo = TempRepo::new("test", None::<&str>, None).unwrap();
        let mut repo = temprepo.repo;
        assert!(repo.config.masters().is_empty());
        repo.config.set("masters", "a b c");
        repo.config.write(None).unwrap();
        let test_repo = Repo::from_path(repo.id, repo.path).unwrap();
        assert_eq!(test_repo.config.masters(), ["a", "b", "c"]);
    }

    #[test]
    fn test_invalid_layout() {
        let temprepo = TempRepo::new("test", None::<&str>, None).unwrap();
        let repo = temprepo.repo;
        repo.config.write(Some("data")).unwrap();
        let r = Repo::from_path(repo.id, repo.path);
        assert_err_re!(r, format!("^.* invalid repo layout: .*$"));
    }

    #[test]
    fn test_id() {
        let temprepo = TempRepo::new("test", None::<&str>, None).unwrap();
        assert_eq!(temprepo.id(), "test");
        assert_eq!(temprepo.repo.id(), "test");
    }

    #[test]
    fn test_categories() {
        let temprepo = TempRepo::new("test", None::<&str>, None).unwrap();
        let repo = temprepo.repo;
        assert_eq!(repo.categories(), Vec::<String>::new());
        fs::create_dir(repo.path.join("cat")).unwrap();
        assert_eq!(repo.categories(), ["cat"]);
        fs::create_dir(repo.path.join("a-cat")).unwrap();
        fs::create_dir(repo.path.join("z-cat")).unwrap();
        assert_eq!(repo.categories(), ["a-cat", "cat", "z-cat"]);
    }

    #[test]
    fn test_packages() {
        let temprepo = TempRepo::new("test", None::<&str>, None).unwrap();
        let repo = temprepo.repo;
        assert_eq!(repo.packages("cat"), Vec::<String>::new());
        fs::create_dir_all(repo.path.join("cat/pkg")).unwrap();
        assert_eq!(repo.packages("cat"), ["pkg"]);
        fs::create_dir_all(repo.path.join("a-cat/pkg-z")).unwrap();
        fs::create_dir_all(repo.path.join("a-cat/pkg-a")).unwrap();
        assert_eq!(repo.packages("a-cat"), ["pkg-a", "pkg-z"]);
    }

    #[test]
    fn test_versions() {
        let temprepo = TempRepo::new("test", None::<&str>, None).unwrap();
        let repo = temprepo.repo;
        assert_eq!(repo.versions("cat", "pkg"), Vec::<String>::new());
        fs::create_dir_all(repo.path.join("cat/pkg")).unwrap();
        fs::File::create(repo.path.join("cat/pkg/pkg-1.ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), ["1"]);

        // unmatching ebuilds are ignored
        fs::File::create(repo.path.join("cat/pkg/foo-2.ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), ["1"]);

        // wrongly named files are ignored
        fs::File::create(repo.path.join("cat/pkg/pkg-2.txt")).unwrap();
        fs::File::create(repo.path.join("cat/pkg/pkg-2..ebuild")).unwrap();
        fs::File::create(repo.path.join("cat/pkg/pkg-2ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), ["1"]);

        fs::File::create(repo.path.join("cat/pkg/pkg-2.ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), ["1", "2"]);

        fs::create_dir_all(repo.path.join("a-cat/pkg10a")).unwrap();
        fs::File::create(repo.path.join("a-cat/pkg10a/pkg10a-0-r0.ebuild")).unwrap();
        assert_eq!(repo.versions("a-cat", "pkg10a"), ["0-r0"]);
    }
}
