use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, fmt, fs, io};

#[cfg(test)]
use std::{collections::HashMap, io::Write};

use ini::Ini;
use once_cell::sync::Lazy;
use tempfile::TempDir;
use tracing::warn;
use walkdir::DirEntry;

use crate::config::Config;
use crate::files::{has_ext, is_dir, is_file, is_hidden, sorted_dir_list};
use crate::macros::build_from_paths;
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
        let mut nonexistent = vec![];
        for id in self.config.masters() {
            match config.repos.repos.get(&id) {
                Some(r) => masters.push(r.clone()),
                None => nonexistent.push(id),
            }
        }

        match nonexistent.is_empty() {
            true => Ok(masters),
            false => {
                let masters = nonexistent.join(", ");
                Err(Error::InvalidRepo {
                    path: self.path.clone(),
                    error: format!("nonexistent masters: {masters}"),
                })
            }
        }
    }

    pub fn trees(&self) -> Result<Vec<Arc<repo::Repository>>> {
        let config = Config::current();
        let mut trees = self.masters()?;
        match config.repos.repos.get(&self.id) {
            Some(r) => {
                trees.push(r.clone());
                Ok(trees)
            }
            None => Err(Error::InvalidRepo {
                path: self.path.clone(),
                error: format!("unconfigured repo: {}", self.id),
            }),
        }
    }

    pub fn category_dirs(&self) -> Vec<String> {
        // filter out non-category dirs
        let filter = |e: &DirEntry| -> bool { is_dir(e) && !is_hidden(e) && !is_fake_category(e) };
        let cats = sorted_dir_list(&self.path).into_iter().filter_entry(filter);
        let mut v = vec![];
        for entry in cats {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("error walking {:?}: {e}", &self.path);
                    continue;
                }
            };
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
        let pkgs = sorted_dir_list(&path).into_iter().filter_entry(filter);
        let mut v = vec![];
        for entry in pkgs {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("error walking {:?}: {e}", &path);
                    continue;
                }
            };
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
        let ebuilds = sorted_dir_list(&path).into_iter().filter_entry(filter);
        let mut v = vec![];
        for entry in ebuilds {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("error walking {:?}: {e}", &path);
                    continue;
                }
            };
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

    fn len(&self) -> usize {
        unimplemented!()
    }

    fn is_empty(&self) -> bool {
        unimplemented!()
    }
}

impl<T: AsRef<Path>> repo::Contains<T> for Repo {
    fn contains(&self, path: T) -> bool {
        let path = path.as_ref();
        if path.is_absolute() {
            if let (Ok(path), Ok(repo_path)) = (path.canonicalize(), self.path.canonicalize()) {
                path.starts_with(&repo_path) && path.exists()
            } else {
                false
            }
        } else {
            self.path.join(path).exists()
        }
    }
}

/// A temporary repo that is automatically deleted when it goes out of scope.
#[derive(Debug)]
pub(crate) struct TempRepo {
    tempdir: TempDir,
    pub(crate) repo: Repo,
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
        let eapi = format!("{}", eapi.unwrap_or(&eapi::EAPI_LATEST));
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

    /// Create an ebuild file in the repo.
    #[cfg(test)]
    pub(crate) fn create_ebuild(
        &self,
        cpv: &str,
        data: Option<HashMap<&str, &str>>,
    ) -> Result<PathBuf> {
        let cpv = atom::parse::cpv(cpv)?;
        let path = self.tempdir.path().join(format!(
            "{}/{}-{}.ebuild",
            cpv.key(),
            cpv.package(),
            cpv.version().unwrap()
        ));
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|e| Error::IO(format!("failed creating {cpv} dir: {e}")))?;
        let mut f = fs::File::create(&path)
            .map_err(|e| Error::IO(format!("failed creating {cpv} ebuild: {e}")))?;

        let data = data.unwrap_or_else(|| HashMap::<&str, &str>::new());
        let eapi = data
            .get("eapi")
            .cloned()
            .unwrap_or(eapi::EAPI_LATEST.as_str());
        let slot = data.get("slot").cloned().unwrap_or("0");

        let content = indoc::formatdoc! {"
            EAPI=\"{eapi}\"
            SLOT=\"{slot}\"
        "};
        f.write_all(content.as_bytes())
            .map_err(|e| Error::IO(format!("failed writing to {cpv} ebuild: {e}")))?;

        Ok(path)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::macros::assert_err_re;
    use crate::repo::{Contains, Repo as RepoTrait};

    use super::*;

    #[test]
    fn test_masters() {
        let t = TempRepo::new("test", None::<&str>, None).unwrap();
        let mut repo = t.repo;
        assert!(repo.config.masters().is_empty());
        repo.config.set("masters", "a b c");
        repo.config.write(None).unwrap();
        let test_repo = Repo::from_path(repo.id, repo.path).unwrap();
        assert_eq!(test_repo.config.masters(), ["a", "b", "c"]);
        // repos don't exist so they'll be flagged if actually trying to access them
        let r = test_repo.masters();
        assert_err_re!(r, format!("^.* nonexistent masters: a, b, c$"));
    }

    #[test]
    fn test_invalid_layout() {
        let t = TempRepo::new("test", None::<&str>, None).unwrap();
        t.repo.config.write(Some("data")).unwrap();
        let r = Repo::from_path(t.repo.id, t.repo.path);
        assert_err_re!(r, format!("^.* invalid repo layout: .*$"));
    }

    #[test]
    fn test_id() {
        let t = TempRepo::new("test", None::<&str>, None).unwrap();
        assert_eq!(t.repo.id(), "test");
    }

    #[test]
    fn test_categories() {
        let t = TempRepo::new("test", None::<&str>, None).unwrap();
        assert_eq!(t.repo.categories(), Vec::<String>::new());
        fs::create_dir(t.repo.path.join("cat")).unwrap();
        assert_eq!(t.repo.categories(), ["cat"]);
        fs::create_dir(t.repo.path.join("a-cat")).unwrap();
        fs::create_dir(t.repo.path.join("z-cat")).unwrap();
        assert_eq!(t.repo.categories(), ["a-cat", "cat", "z-cat"]);
    }

    #[test]
    fn test_packages() {
        let t = TempRepo::new("test", None::<&str>, None).unwrap();
        assert_eq!(t.repo.packages("cat"), Vec::<String>::new());
        fs::create_dir_all(t.repo.path.join("cat/pkg")).unwrap();
        assert_eq!(t.repo.packages("cat"), ["pkg"]);
        fs::create_dir_all(t.repo.path.join("a-cat/pkg-z")).unwrap();
        fs::create_dir_all(t.repo.path.join("a-cat/pkg-a")).unwrap();
        assert_eq!(t.repo.packages("a-cat"), ["pkg-a", "pkg-z"]);
    }

    #[test]
    fn test_versions() {
        let t = TempRepo::new("test", None::<&str>, None).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), Vec::<String>::new());
        fs::create_dir_all(t.repo.path.join("cat/pkg")).unwrap();
        fs::File::create(t.repo.path.join("cat/pkg/pkg-1.ebuild")).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), ["1"]);

        // unmatching ebuilds are ignored
        fs::File::create(t.repo.path.join("cat/pkg/foo-2.ebuild")).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), ["1"]);

        // wrongly named files are ignored
        fs::File::create(t.repo.path.join("cat/pkg/pkg-2.txt")).unwrap();
        fs::File::create(t.repo.path.join("cat/pkg/pkg-2..ebuild")).unwrap();
        fs::File::create(t.repo.path.join("cat/pkg/pkg-2ebuild")).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), ["1"]);

        fs::File::create(t.repo.path.join("cat/pkg/pkg-2.ebuild")).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), ["1", "2"]);

        fs::create_dir_all(t.repo.path.join("a-cat/pkg10a")).unwrap();
        fs::File::create(t.repo.path.join("a-cat/pkg10a/pkg10a-0-r0.ebuild")).unwrap();
        assert_eq!(t.repo.versions("a-cat", "pkg10a"), ["0-r0"]);
    }

    #[test]
    fn test_contains_path() {
        let t = TempRepo::new("test", None::<&str>, None).unwrap();
        assert!(!t.repo.contains("cat/pkg"));
        t.create_ebuild("cat/pkg-1", None).unwrap();
        assert!(t.repo.contains("cat/pkg"));
        assert!(t.repo.contains("cat/pkg/pkg-1.ebuild"));
        assert!(!t.repo.contains("pkg-1.ebuild"));
    }
}
