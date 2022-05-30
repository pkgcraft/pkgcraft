use std::collections::HashSet;
use std::iter::Flatten;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, fmt, fs, io};

#[cfg(test)]
use std::io::Write;

use ini::Ini;
use once_cell::sync::Lazy;
use regex::Regex;
use tempfile::TempDir;
use tracing::warn;
use walkdir::WalkDir;

use super::{make_repo_traits, Repository};
use crate::config::{Config, RepoConfig};
use crate::files::{has_ext, is_dir, is_file, is_hidden, sorted_dir_list};
use crate::macros::build_from_paths;
use crate::pkg::Package;
use crate::{atom, eapi, pkg, repo, Error};

static EBUILD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<cat>[^/]+)/(?P<pkg>[^/]+)/(?P<p>[^/]+).ebuild$").unwrap());
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
    fn new<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
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
    fn write(&self, data: Option<&str>) -> crate::Result<()> {
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
    config: RepoConfig,
    pub(super) meta: Metadata,
}

make_repo_traits!(Repo);

impl Repo {
    pub(super) const FORMAT: &'static str = "ebuild";

    fn new<S>(id: S, config: Option<RepoConfig>, meta: Metadata) -> crate::Result<Self>
    where
        S: AsRef<str>,
    {
        Ok(Repo {
            id: id.as_ref().to_string(),
            config: config.unwrap_or_default(),
            meta,
        })
    }

    pub(super) fn from_path<S, P>(id: S, priority: i32, path: P) -> crate::Result<Self>
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

        let meta =
            Metadata::new(path.join("metadata/layout.conf")).map_err(|e| Error::InvalidRepo {
                path: PathBuf::from(path),
                error: e.to_string(),
            })?;

        let config = RepoConfig {
            location: PathBuf::from(path),
            priority,
            ..Default::default()
        };

        Repo::new(id, Some(config), meta)
    }

    pub fn masters(&self) -> crate::Result<Vec<Arc<repo::Repo>>> {
        let config = Config::current();
        let mut masters = vec![];
        let mut nonexistent = vec![];
        for id in self.meta.masters() {
            match config.repos.get(&id) {
                Some(r) => masters.push(r.clone()),
                None => nonexistent.push(id),
            }
        }

        match nonexistent.is_empty() {
            true => Ok(masters),
            false => {
                let masters = nonexistent.join(", ");
                Err(Error::InvalidRepo {
                    path: self.path().into(),
                    error: format!("nonexistent masters: {masters}"),
                })
            }
        }
    }

    pub fn trees(&self) -> crate::Result<Vec<Arc<repo::Repo>>> {
        let config = Config::current();
        let mut trees = self.masters()?;
        match config.repos.get(&self.id) {
            Some(r) => {
                trees.push(r.clone());
                Ok(trees)
            }
            None => Err(Error::InvalidRepo {
                path: self.path().into(),
                error: format!("unconfigured repo: {}", self.id),
            }),
        }
    }

    pub fn category_dirs(&self) -> Vec<String> {
        // filter out non-category dirs
        let filter =
            |e: &walkdir::DirEntry| -> bool { is_dir(e) && !is_hidden(e) && !is_fake_category(e) };
        let cats = sorted_dir_list(self.path())
            .into_iter()
            .filter_entry(filter);
        let mut v = vec![];
        for entry in cats {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("error walking {:?}: {e}", self.path());
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

    /// Convert an ebuild path inside the repo into an Atom.
    pub(crate) fn atom_from_path(&self, path: &Path) -> crate::Result<atom::Atom> {
        let err = |s: &str| -> Error {
            Error::InvalidValue(format!("invalid ebuild path: {path:?}: {s}"))
        };
        path.strip_prefix(self.path())
            .map_err(|_| err("missing repo prefix"))
            .and_then(|p| p.to_str().ok_or_else(|| err("non-unicode")))
            .and_then(|s| EBUILD_RE.captures(s).ok_or_else(|| err("unmatched file")))
            .and_then(|m| {
                let cat = m.name("cat").unwrap().as_str();
                let pkg = m.name("pkg").unwrap().as_str();
                let p = m.name("p").unwrap().as_str();
                atom::parse::cpv(&format!("{cat}/{p}"))
                    .map_err(|_| err("invalid CPV"))
                    .and_then(|a| match a.package() == pkg {
                        true => Ok(a),
                        false => Err(err("mismatched package dir")),
                    })
            })
    }

    pub fn iter(&self) -> PkgIter {
        self.into_iter()
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (id, path) = (self.id.as_str(), self.path().to_string_lossy());
        match id == path {
            true => write!(f, "{id}"),
            false => write!(f, "{id}: {path}"),
        }
    }
}

fn is_fake_category(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| FAKE_CATEGORIES.contains(s))
        .unwrap_or(false)
}

impl Repository for Repo {
    fn categories(&self) -> Vec<String> {
        // TODO: implement reading profiles/categories, falling back to category_dirs()
        self.category_dirs()
    }

    fn packages(&self, cat: &str) -> Vec<String> {
        let path = self.path().join(cat.strip_prefix('/').unwrap_or(cat));
        let filter = |e: &walkdir::DirEntry| -> bool { is_dir(e) && !is_hidden(e) };
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
            self.path(),
            cat.strip_prefix('/').unwrap_or(cat),
            pkg.strip_prefix('/').unwrap_or(pkg)
        );
        let ebuilds = sorted_dir_list(&path)
            .into_iter()
            .filter_entry(ebuild_filter);
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
                        Ok(ver) => v.push(format!("{ver}")),
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

    fn config(&self) -> &RepoConfig {
        &self.config
    }

    fn len(&self) -> usize {
        self.iter().count()
    }

    fn is_empty(&self) -> bool {
        self.iter().count() == 0
    }
}

impl<T: AsRef<Path>> repo::Contains<T> for Repo {
    fn contains(&self, path: T) -> bool {
        let path = path.as_ref();
        if path.is_absolute() {
            if let (Ok(path), Ok(repo_path)) = (path.canonicalize(), self.path().canonicalize()) {
                path.starts_with(&repo_path) && path.exists()
            } else {
                false
            }
        } else {
            self.path().join(path).exists()
        }
    }
}

impl repo::Contains<atom::Atom> for Repo {
    fn contains(&self, atom: atom::Atom) -> bool {
        self.iter().any(|p| p.atom() == &atom)
    }
}

impl repo::Contains<&atom::Atom> for Repo {
    fn contains(&self, atom: &atom::Atom) -> bool {
        self.iter().any(|p| p.atom() == atom)
    }
}

fn ebuild_filter(e: &walkdir::DirEntry) -> bool {
    is_file(e) && !is_hidden(e) && has_ext(e, "ebuild")
}

impl<'a> IntoIterator for &'a Repo {
    type Item = pkg::ebuild::Pkg<'a>;
    type IntoIter = PkgIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        #[allow(clippy::needless_collect)]
        let ebuilds: Vec<WalkDir> = self
            .categories()
            .iter()
            .map(|d| {
                WalkDir::new(self.path().join(d))
                    .sort_by_file_name()
                    .min_depth(2)
                    .max_depth(2)
            })
            .collect();

        PkgIter {
            iter: ebuilds.into_iter().flatten(),
            repo: self,
        }
    }
}

pub struct PkgIter<'a> {
    iter: Flatten<std::vec::IntoIter<WalkDir>>,
    repo: &'a Repo,
}

impl<'a> Iterator for PkgIter<'a> {
    type Item = pkg::ebuild::Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(Ok(e)) => {
                    if ebuild_filter(&e) {
                        let path = e.path();
                        match pkg::ebuild::Pkg::new(path, self.repo) {
                            Ok(p) => return Some(p),
                            Err(e) => warn!("{} repo: invalid pkg: {path:?}: {e}", self.repo.id),
                        }
                    }
                }
                Some(Err(e)) => warn!("{}: failed walking repo: {e}", self.repo.id),
                None => return None,
            }
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
        priority: i32,
        path: Option<P>,
        eapi: Option<&eapi::Eapi>,
    ) -> crate::Result<Self> {
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

        let repo = Repo::from_path(id, priority, temp_path)?;
        Ok(TempRepo { tempdir, repo })
    }

    /// Create an ebuild file in the repo.
    #[cfg(test)]
    pub(crate) fn create_ebuild<'a, I>(&self, cpv: &str, data: I) -> crate::Result<PathBuf>
    where
        I: IntoIterator<Item = (eapi::Key, &'a str)>,
    {
        use crate::eapi::Key::*;
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

        Ok(path)
    }

    /// Attempts to persist the temporary repo to disk, returning the [`PathBuf`] where it is
    /// located.
    pub(crate) fn persist<P: AsRef<Path>>(self, path: Option<P>) -> crate::Result<PathBuf> {
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

    use rusty_fork::rusty_fork_test;
    use tracing_test::traced_test;

    use crate::eapi::Key::*;
    use crate::macros::{assert_err_re, assert_logs_re};
    use crate::repo::{Contains, Repository};

    use super::*;

    #[test]
    fn test_masters() {
        let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
        let mut repo = t.repo;
        assert!(repo.meta.masters().is_empty());
        repo.meta.set("masters", "a b c");
        repo.meta.write(None).unwrap();
        let test_repo = Repo::from_path(repo.id(), 0, repo.path()).unwrap();
        assert_eq!(test_repo.meta.masters(), ["a", "b", "c"]);
        // repos don't exist so they'll be flagged if actually trying to access them
        let r = test_repo.masters();
        assert_err_re!(r, format!("^.* nonexistent masters: a, b, c$"));
    }

    #[test]
    fn test_invalid_layout() {
        let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
        t.repo.meta.write(Some("data")).unwrap();
        let r = Repo::from_path(t.repo.id(), 0, t.repo.path());
        assert_err_re!(r, format!("^.* invalid repo layout: .*$"));
    }

    #[test]
    fn test_id() {
        let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
        assert_eq!(t.repo.id(), "test");
    }

    #[test]
    fn test_len() {
        let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
        assert_eq!(t.repo.len(), 0);
        assert!(t.repo.is_empty());
        t.create_ebuild("cat/pkg-1", []).unwrap();
        assert_eq!(t.repo.len(), 1);
        assert!(!t.repo.is_empty());
        t.create_ebuild("cat2/pkg-1", []).unwrap();
        assert_eq!(t.repo.len(), 2);
        assert!(!t.repo.is_empty());
    }

    #[test]
    fn test_categories() {
        let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
        assert_eq!(t.repo.categories(), Vec::<String>::new());
        fs::create_dir(t.repo.path().join("cat")).unwrap();
        assert_eq!(t.repo.categories(), ["cat"]);
        fs::create_dir(t.repo.path().join("a-cat")).unwrap();
        fs::create_dir(t.repo.path().join("z-cat")).unwrap();
        assert_eq!(t.repo.categories(), ["a-cat", "cat", "z-cat"]);
    }

    #[test]
    fn test_packages() {
        let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
        assert_eq!(t.repo.packages("cat"), Vec::<String>::new());
        fs::create_dir_all(t.repo.path().join("cat/pkg")).unwrap();
        assert_eq!(t.repo.packages("cat"), ["pkg"]);
        fs::create_dir_all(t.repo.path().join("a-cat/pkg-z")).unwrap();
        fs::create_dir_all(t.repo.path().join("a-cat/pkg-a")).unwrap();
        assert_eq!(t.repo.packages("a-cat"), ["pkg-a", "pkg-z"]);
    }

    #[test]
    fn test_versions() {
        let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), Vec::<String>::new());
        fs::create_dir_all(t.repo.path().join("cat/pkg")).unwrap();
        fs::File::create(t.repo.path().join("cat/pkg/pkg-1.ebuild")).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), ["1"]);

        // unmatching ebuilds are ignored
        fs::File::create(t.repo.path().join("cat/pkg/foo-2.ebuild")).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), ["1"]);

        // wrongly named files are ignored
        fs::File::create(t.repo.path().join("cat/pkg/pkg-2.txt")).unwrap();
        fs::File::create(t.repo.path().join("cat/pkg/pkg-2..ebuild")).unwrap();
        fs::File::create(t.repo.path().join("cat/pkg/pkg-2ebuild")).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), ["1"]);

        fs::File::create(t.repo.path().join("cat/pkg/pkg-2.ebuild")).unwrap();
        assert_eq!(t.repo.versions("cat", "pkg"), ["1", "2"]);

        fs::create_dir_all(t.repo.path().join("a-cat/pkg10a")).unwrap();
        fs::File::create(t.repo.path().join("a-cat/pkg10a/pkg10a-0-r0.ebuild")).unwrap();
        assert_eq!(t.repo.versions("a-cat", "pkg10a"), ["0-r0"]);
    }

    #[test]
    fn test_contains_path() {
        let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
        assert!(!t.repo.contains("cat/pkg"));
        t.create_ebuild("cat/pkg-1", []).unwrap();
        assert!(t.repo.contains("cat/pkg"));
        assert!(t.repo.contains("cat/pkg/pkg-1.ebuild"));
        assert!(!t.repo.contains("pkg-1.ebuild"));
    }

    // TODO: drop this once bash process pool support is added
    rusty_fork_test! {
        #[test]
        fn test_iter() {
            let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
            t.create_ebuild("cat2/pkg-1", []).unwrap();
            t.create_ebuild("cat1/pkg-1", []).unwrap();
            let mut iter = t.repo.iter();
            for cpv in ["cat1/pkg-1", "cat2/pkg-1"] {
                let pkg = iter.next();
                assert_eq!(pkg.map(|p| format!("{}", p.atom())), Some(cpv.to_string()));
            }
            assert!(iter.next().is_none());
        }

        #[traced_test]
        #[test]
        fn test_invalid_pkgs() {
            for (data, err) in [
                    ([(Eapi, "-1")], "invalid EAPI: -1"),
                    ([(Eapi, "a")], "unknown EAPI: a"),
                    ([(Slot, "-")], "missing required value: SLOT"),
                    ] {
                let t = TempRepo::new("test", 0, None::<&str>, None).unwrap();
                t.create_ebuild("cat/pkg-0", data).unwrap();
                let mut iter = t.repo.iter();
                assert!(iter.next().is_none());
                assert_logs_re!(format!("test repo: invalid pkg: .+: {err}$"));
            }
        }
    }
}
