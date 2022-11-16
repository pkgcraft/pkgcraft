use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::SplitWhitespace;
use std::sync::{Arc, Weak};
use std::{env, fmt, fs, io, iter, thread};

use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use indexmap::IndexSet;
use ini::Ini;
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use tempfile::TempDir;
use tracing::warn;
use walkdir::{DirEntry, WalkDir};

use super::{make_repo_traits, Contains, PkgRepository, Repository};
use crate::config::{self, RepoConfig};
use crate::files::{has_ext, is_dir, is_file, is_hidden, sorted_dir_list};
use crate::macros::build_from_paths;
use crate::metadata::ebuild::{Manifest, XmlMetadata};
use crate::pkg::ebuild::Pkg;
use crate::restrict::{Restrict, Restriction, Str};
use crate::{atom, eapi, repo, Error};

static EBUILD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<cat>[^/]+)/(?P<pkg>[^/]+)/(?P<p>[^/]+).ebuild$").unwrap());
const DEFAULT_SECTION: Option<String> = None;
static FAKE_CATEGORIES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["eclass", "profiles", "metadata", "licenses"]
        .into_iter()
        .collect()
});

pub struct Config {
    path: Option<Utf8PathBuf>,
    ini: Ini,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            path: None,
            ini: Ini::new(),
        }
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let section = self.ini.section(DEFAULT_SECTION);
        f.debug_struct("Metadata")
            .field("path", &self.path)
            .field("ini", &section)
            .finish()
    }
}

impl Config {
    fn new(repo_path: &Utf8Path) -> crate::Result<Self> {
        let path = repo_path.join("metadata/layout.conf");
        match Ini::load_from_file(&path) {
            Ok(ini) => Ok(Self {
                path: Some(path),
                ini,
            }),
            Err(ini::Error::Io(e)) if e.kind() == io::ErrorKind::NotFound => Ok(Self {
                path: Some(path),
                ini: Ini::new(),
            }),
            Err(e) => Err(Error::InvalidValue(format!("invalid repo config: {path:?}: {e}"))),
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

    fn iter(&self, key: &str) -> SplitWhitespace {
        self.ini
            .get_from(DEFAULT_SECTION, key)
            .unwrap_or_default()
            .split_whitespace()
    }

    pub fn properties_allowed(&self) -> HashSet<&str> {
        self.iter("properties-allowed").collect()
    }

    pub fn restrict_allowed(&self) -> HashSet<&str> {
        self.iter("restrict-allowed").collect()
    }
}

#[derive(Debug, Default)]
pub(crate) struct Metadata {
    profiles_base: Utf8PathBuf,
    arches: OnceCell<IndexSet<String>>,
}

impl Metadata {
    fn new(repo_path: &Utf8Path) -> Self {
        Self {
            profiles_base: repo_path.join("profiles"),
            ..Default::default()
        }
    }

    fn arches(&self) -> &IndexSet<String> {
        self.arches.get_or_init(|| {
            let path = self.profiles_base.join("arch.list");
            match fs::read_to_string(&path) {
                Ok(s) => s
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.starts_with('#'))
                    .map(String::from)
                    .collect(),
                Err(_) => IndexSet::new(),
            }
        })
    }
}

/// Shared data cache trait.
pub(crate) trait CacheData {
    fn new(path: &Utf8Path) -> Self;
}

#[derive(Debug)]
struct Cache<T>
where
    T: CacheData + Send + Sync,
{
    thread: Option<thread::JoinHandle<()>>,
    sender: Sender<Msg>,
    receiver: Receiver<Arc<T>>,
}

enum Msg {
    Key(String),
    Stop,
}

impl<T> Cache<T>
where
    T: CacheData + Send + Sync + 'static,
{
    fn new(repo: &Repo) -> Cache<T> {
        let (path_sender, path_receiver) = bounded::<Msg>(10);
        let (meta_sender, meta_receiver) = bounded::<Arc<T>>(10);
        let path = Utf8PathBuf::from(repo.path());

        let thread = thread::spawn(move || {
            let repo_path = path;
            let mut pkg_data = HashMap::<String, Arc<T>>::new();
            loop {
                match path_receiver.recv() {
                    Ok(Msg::Stop) | Err(RecvError) => break,
                    Ok(Msg::Key(s)) => {
                        // TODO: evict cache entries based on file modification time
                        let data = match pkg_data.get(&s) {
                            Some(data) => data.clone(),
                            None => {
                                let path = repo_path.join(&s);
                                let data = Arc::new(T::new(&path));
                                pkg_data.insert(s, data.clone());
                                data
                            }
                        };
                        meta_sender
                            .send(data)
                            .expect("failed sending shared pkg data");
                    }
                }
            }
        });

        Self {
            thread: Some(thread),
            sender: path_sender,
            receiver: meta_receiver,
        }
    }
}

// Note that the thread will currently be killed without joining on exit since
// Cache is contained in a OnceCell that doesn't call drop().
impl<T> Drop for Cache<T>
where
    T: CacheData + Send + Sync,
{
    fn drop(&mut self) {
        self.sender.send(Msg::Stop).unwrap();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

#[derive(Default)]
pub struct Repo {
    id: String,
    repo_config: RepoConfig,
    config: Config,
    metadata: Metadata,
    profiles_base: Utf8PathBuf,
    name: String,
    masters: OnceCell<Vec<Weak<Repo>>>,
    trees: OnceCell<Vec<Weak<Repo>>>,
    eclasses: OnceCell<HashMap<String, Utf8PathBuf>>,
    xml_cache: OnceCell<Cache<XmlMetadata>>,
    manifest_cache: OnceCell<Cache<Manifest>>,
}

impl fmt::Debug for Repo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Repo")
            .field("id", &self.id)
            .field("repo_config", &self.repo_config)
            .field("name", &self.name)
            .finish()
    }
}

make_repo_traits!(Repo);

impl Repo {
    pub(super) fn from_path<S, P>(id: S, priority: i32, path: P) -> crate::Result<Self>
    where
        S: AsRef<str>,
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref();
        let profiles_base = path.join("profiles");

        let invalid_repo = |err: String| -> Error {
            Error::InvalidRepo {
                path: Utf8PathBuf::from(path),
                err,
            }
        };

        if !profiles_base.exists() {
            return Err(invalid_repo("missing profiles dir".to_string()));
        }

        let repo_name_path = profiles_base.join("repo_name");
        let name = match fs::read_to_string(&repo_name_path) {
            Ok(data) => match data.lines().next() {
                // TODO: verify repo name matches spec
                Some(s) => s.trim_end().to_string(),
                None => {
                    let err = format!("invalid repo name: {:?}", &repo_name_path);
                    return Err(invalid_repo(err));
                }
            },
            Err(e) => {
                let err = format!("missing repo name: {:?}: {e}", &repo_name_path);
                return Err(invalid_repo(err));
            }
        };

        let repo_config = RepoConfig {
            location: Utf8PathBuf::from(path),
            priority,
            ..Default::default()
        };

        let config = Config::new(path).map_err(|e| invalid_repo(e.to_string()))?;

        Ok(Self {
            id: id.as_ref().to_string(),
            repo_config,
            config,
            metadata: Metadata::new(path),
            profiles_base,
            name,
            ..Default::default()
        })
    }

    pub(super) fn finalize(&self) -> crate::Result<()> {
        let config = config::Config::current();
        let mut nonexistent = vec![];
        let mut masters = vec![];

        for id in self.config.iter("masters") {
            match config.repos.get(id) {
                Some(repo::Repo::Ebuild(r)) => masters.push(Arc::downgrade(r)),
                _ => nonexistent.push(id),
            }
        }

        match nonexistent.is_empty() {
            true => {
                if self.masters.set(masters).is_err() {
                    panic!("masters already set: {}", self.id());
                }
                Ok(())
            }
            false => {
                let repos = nonexistent.join(", ");
                Err(Error::InvalidRepo {
                    path: self.path().into(),
                    err: format!("unconfigured repos: {repos}"),
                })
            }
        }
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.repo_config
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Return the list of inherited repos.
    pub fn masters(&self) -> Vec<Arc<Repo>> {
        self.masters
            .get()
            .expect("finalize() uncalled")
            .iter()
            .map(|p| p.upgrade().expect("unconfigured repo"))
            .collect()
    }

    /// Return a repo's inheritance list including itself.
    pub fn trees(&self) -> Vec<Arc<Repo>> {
        self.trees
            .get_or_init(|| {
                let config = config::Config::current();
                let mut trees = self.masters();
                match config.repos.get(self.id()) {
                    Some(repo::Repo::Ebuild(r)) => trees.push(r.clone()),
                    _ => panic!("unconfigured repo: {}", self.id()),
                }
                trees.iter().map(Arc::downgrade).collect()
            })
            .iter()
            .map(|p| p.upgrade().expect("unconfigured repo"))
            .collect()
    }

    /// Return the mapping of inherited eclass names to file paths.
    pub fn eclasses(&self) -> &HashMap<String, Utf8PathBuf> {
        self.eclasses.get_or_init(|| {
            self.trees()
                .iter()
                .filter_map(|repo| repo.path().join("eclass").read_dir_utf8().ok())
                .flatten()
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    match (path.file_stem(), path.extension()) {
                        (Some(f), Some("eclass")) => Some((f.to_string(), path.to_path_buf())),
                        _ => None,
                    }
                })
                .collect()
        })
    }

    /// Return a repo's category dirs from the filesystem.
    pub fn category_dirs(&self) -> Vec<String> {
        // filter out non-category dirs
        let filter = |e: &DirEntry| -> bool { is_dir(e) && !is_hidden(e) && !is_fake_category(e) };
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

    /// Return a repo's configured categories from the `profiles/categories` file.
    fn pms_categories(&self) -> Vec<String> {
        let mut cats = vec![];
        if let Ok(data) = fs::read_to_string(self.profiles_base.join("categories")) {
            cats.extend(data.lines().map(|s| s.to_string()));
        }
        cats
    }

    /// Convert an ebuild path inside the repo into an Atom.
    pub(crate) fn atom_from_path(&self, path: &Utf8Path) -> crate::Result<atom::Atom> {
        let err = |s: &str| -> Error {
            Error::InvalidValue(format!("invalid ebuild path: {path:?}: {s}"))
        };
        path.strip_prefix(self.path())
            .map_err(|_| err(&format!("missing repo prefix: {:?}", self.path())))
            .and_then(|p| {
                EBUILD_RE
                    .captures(p.as_str())
                    .ok_or_else(|| err("unmatched file"))
            })
            .and_then(|m| {
                let cat = m.name("cat").unwrap().as_str();
                let pkg = m.name("pkg").unwrap().as_str();
                let p = m.name("p").unwrap().as_str();
                atom::cpv(&format!("{cat}/{p}"))
                    .map_err(|_| err("invalid CPV"))
                    .and_then(|a| match a.package() == pkg {
                        true => Ok(a),
                        false => Err(err("mismatched package dir")),
                    })
            })
    }

    fn xml_cache(&self) -> &Cache<XmlMetadata> {
        self.xml_cache
            .get_or_init(|| Cache::<XmlMetadata>::new(self))
    }

    fn manifest_cache(&self) -> &Cache<Manifest> {
        self.manifest_cache
            .get_or_init(|| Cache::<Manifest>::new(self))
    }

    pub(crate) fn pkg_xml(&self, cpv: &atom::Atom) -> Arc<XmlMetadata> {
        let key = format!("{}/{}", cpv.category(), cpv.package());
        self.xml_cache()
            .sender
            .send(Msg::Key(key))
            .expect("failed requesting pkg xml data");
        self.xml_cache()
            .receiver
            .recv()
            .expect("failed receiving pkg xml data")
    }

    pub(crate) fn pkg_manifest(&self, cpv: &atom::Atom) -> Arc<Manifest> {
        let key = format!("{}/{}", cpv.category(), cpv.package());
        self.manifest_cache()
            .sender
            .send(Msg::Key(key))
            .expect("failed requesting pkg manifest data");
        self.manifest_cache()
            .receiver
            .recv()
            .expect("failed receiving pkg manifest data")
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn arches(&self) -> &IndexSet<String> {
        self.metadata.arches()
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (id, path) = (self.id.as_str(), self.path().as_str());
        match id == path {
            true => write!(f, "{id}"),
            false => write!(f, "{id}: {path}"),
        }
    }
}

fn is_fake_category(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| FAKE_CATEGORIES.contains(s))
        .unwrap_or(false)
}

impl PkgRepository for Repo {
    type Pkg<'a> = Pkg<'a> where Self: 'a;
    type Iterator<'a> = PkgIter<'a> where Self: 'a;
    type RestrictIterator<'a> = RestrictPkgIter<'a> where Self: 'a;

    fn categories(&self) -> Vec<String> {
        // use profiles/categories from repos, falling back to raw fs dirs
        let mut categories = HashSet::<String>::new();
        for r in self.trees() {
            categories.extend(r.pms_categories())
        }
        let mut categories: Vec<_> = categories.into_iter().collect();
        categories.sort();
        match categories.is_empty() {
            false => categories,
            true => self.category_dirs(),
        }
    }

    fn packages(&self, cat: &str) -> Vec<String> {
        let path = self.path().join(cat.strip_prefix('/').unwrap_or(cat));
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
            self.path(),
            cat.strip_prefix('/').unwrap_or(cat),
            pkg.strip_prefix('/').unwrap_or(pkg)
        );
        let ebuilds = sorted_dir_list(&path).into_iter().filter_entry(is_ebuild);
        let mut versions = vec![];
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
                    true => match atom::Version::new(&pf[pn.len() + 1..]) {
                        Ok(v) => versions.push(v),
                        Err(e) => warn!("{e}: {path:?}"),
                    },
                    false => warn!("unmatched ebuild: {path:?}"),
                },
                _ => warn!("non-unicode path: {path:?}"),
            }
        }
        versions.sort();
        versions.iter().map(|v| v.to_string()).collect()
    }

    fn len(&self) -> usize {
        self.iter().count()
    }

    fn iter(&self) -> Self::Iterator<'_> {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::RestrictIterator<'_> {
        let restrict = val.into();
        RestrictPkgIter {
            iter: PkgIter::new(self, Some(&restrict)),
            restrict,
        }
    }
}

impl Repository for Repo {
    fn id(&self) -> &str {
        &self.id
    }

    fn priority(&self) -> i32 {
        self.repo_config.priority
    }

    fn path(&self) -> &Utf8Path {
        &self.repo_config.location
    }

    fn sync(&self) -> crate::Result<()> {
        self.repo_config.sync()
    }
}

impl<T: AsRef<Utf8Path>> Contains<T> for Repo {
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

fn is_ebuild(e: &DirEntry) -> bool {
    is_file(e) && !is_hidden(e) && has_ext(e, "ebuild")
}

impl<'a> IntoIterator for &'a Repo {
    type Item = Pkg<'a>;
    type IntoIter = PkgIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PkgIter::new(self, None)
    }
}

pub struct PkgIter<'a> {
    iter: Box<dyn Iterator<Item = (Utf8PathBuf, atom::Atom)> + 'a>,
    repo: &'a Repo,
}

impl<'a> PkgIter<'a> {
    fn new(repo: &'a Repo, restrict: Option<&Restrict>) -> Self {
        use crate::atom::Restrict::{And, Category, Package, Version};
        let mut cat_restricts = vec![];
        let mut pkg_restricts = vec![];
        let (mut cat, mut pkg, mut ver) = (None, None, None);

        // extract atom restrictions for package filtering
        if let Some(Restrict::Atom(And(vals))) = restrict {
            for r in vals.iter().map(Deref::deref) {
                match r {
                    Category(r) => {
                        cat_restricts.push(r.clone());
                        if let Str::Equal(s) = r {
                            cat = Some(s.to_string());
                        }
                    }
                    r @ Package(x) => {
                        pkg_restricts.push(r.clone());
                        if let Str::Equal(s) = x {
                            pkg = Some(s.to_string());
                        }
                    }
                    r @ Version(x) => {
                        pkg_restricts.push(r.clone());
                        if let Some(v) = x {
                            ver = Some(v.to_string());
                        }
                    }
                    _ => (),
                }
            }
        }

        // filter invalid ebuild paths
        let filter_path = |r: walkdir::Result<DirEntry>| -> Option<(Utf8PathBuf, atom::Atom)> {
            match r {
                Ok(e) => {
                    let path = e.path();
                    match <&Utf8Path>::try_from(path) {
                        Ok(p) => match repo.atom_from_path(p) {
                            Ok(a) => Some((p.to_path_buf(), a)),
                            Err(e) => {
                                warn!("{}: {e}", repo.id);
                                None
                            }
                        },
                        Err(e) => {
                            warn!("{}: invalid unicode path: {path:?}: {e}", repo.id);
                            None
                        }
                    }
                }
                Err(e) => {
                    warn!("{}: failed walking repo: {e}", repo.id);
                    None
                }
            }
        };

        // return valid ebuild (path, atom) tuples in a category
        let category_ebuilds = move |cat: &str| -> Vec<(Utf8PathBuf, atom::Atom)> {
            let mut paths: Vec<_> = WalkDir::new(repo.path().join(cat))
                .min_depth(2)
                .max_depth(2)
                .into_iter()
                .filter_entry(is_ebuild)
                .filter_map(filter_path)
                .collect();
            paths.sort_by(|(_p1, a1), (_p2, a2)| a1.cmp(a2));
            paths
        };

        Self {
            iter: match (cat, pkg, ver) {
                // single atom restriction
                (Some(cat), Some(pkg), Some(ver)) => {
                    let s = format!("{cat}/{pkg}-{ver}");
                    let cpv = atom::cpv(&s).expect("atom restrict failed");
                    let path =
                        build_from_paths!(repo.path(), &cat, &pkg, format!("{pkg}-{ver}.ebuild"));
                    Box::new(iter::once((path, cpv)))
                }

                // complex restriction filtering
                _ => {
                    let cat_restrict = match &cat_restricts[..] {
                        [] => Restrict::True,
                        [_] => cat_restricts.remove(0).into(),
                        _ => Restrict::and(cat_restricts),
                    };

                    let pkg_restrict = match &pkg_restricts[..] {
                        [] => Restrict::True,
                        [_] => pkg_restricts.remove(0).into(),
                        _ => Restrict::and(pkg_restricts),
                    };

                    Box::new(
                        repo.categories()
                            .into_iter()
                            .filter(move |s| cat_restrict.matches(s.as_str()))
                            .flat_map(move |s| category_ebuilds(s.as_str()))
                            .filter(move |(_, atom)| pkg_restrict.matches(atom)),
                    )
                }
            },
            repo,
        }
    }
}

impl<'a> Iterator for PkgIter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        for (path, atom) in &mut self.iter {
            match Pkg::new(path, atom, self.repo) {
                Ok(pkg) => return Some(pkg),
                Err(e) => warn!("{} repo: {e}", self.repo.id),
            }
        }
        None
    }
}

pub struct RestrictPkgIter<'a> {
    iter: PkgIter<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for RestrictPkgIter<'a> {
    type Item = Pkg<'a>;

    #[allow(clippy::manual_find)]
    fn next(&mut self) -> Option<Self::Item> {
        for pkg in &mut self.iter {
            if self.restrict.matches(&pkg) {
                return Some(pkg);
            }
        }
        None
    }
}

/// A temporary repo that is automatically deleted when it goes out of scope.
#[derive(Debug)]
pub struct TempRepo {
    tempdir: TempDir,
    pub(crate) path: Utf8PathBuf,
}

impl TempRepo {
    /// Attempts to create a temporary repo inside an optional path or inside `env::temp_dir()` if
    /// no path is specified.
    pub(crate) fn new(
        id: &str,
        path: Option<&Utf8Path>,
        eapi: Option<&eapi::Eapi>,
    ) -> crate::Result<Self> {
        let path = match path {
            Some(p) => p.to_path_buf().into_std_path_buf(),
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

        let path = Utf8PathBuf::from_path_buf(temp_path.to_path_buf())
            .map_err(|_| Error::RepoInit(format!("non-unicode repo path: {temp_path:?}")))?;
        Ok(TempRepo { tempdir, path })
    }

    /// Create an ebuild file in the repo.
    pub fn create_ebuild<'a, I>(
        &self,
        cpv: &str,
        data: I,
    ) -> crate::Result<(Utf8PathBuf, atom::Atom)>
    where
        I: IntoIterator<Item = (crate::metadata::Key, &'a str)>,
    {
        use crate::metadata::Key::*;
        let cpv = atom::cpv(cpv)?;
        let path = self.path.join(format!(
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

        Ok((path, cpv))
    }

    /// Create an ebuild file in the repo from raw data.
    pub fn create_ebuild_raw(
        &self,
        cpv: &str,
        data: &str,
    ) -> crate::Result<(Utf8PathBuf, atom::Atom)> {
        let cpv = atom::cpv(cpv)?;
        let path = self.path.join(format!(
            "{}/{}-{}.ebuild",
            cpv.key(),
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
        write!(f, "temp repo: {:?}", self.path)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use tracing_test::traced_test;

    use crate::config::Config;
    use crate::macros::{assert_err_re, assert_logs_re};
    use crate::metadata::Key;
    use crate::pkg::Package;
    use crate::test::eq_sorted;

    use super::*;

    #[test]
    fn test_masters() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();

        // nonexistent
        let t = TempRepo::new("test", None, None).unwrap();
        let mut repo = Repo::from_path("test", 0, t.path).unwrap();
        repo.config.set("masters", "a b c");
        repo.config.write(None).unwrap();
        let r = config.add_repo_path(repo.id(), 0, repo.path().as_str());
        assert_err_re!(r, format!("^.* unconfigured repos: a, b, c$"));

        // none
        let t = TempRepo::new("a", None, None).unwrap();
        let repo = Repo::from_path("a", 0, t.path).unwrap();
        config
            .add_repo_path(repo.id(), 0, repo.path().as_str())
            .unwrap();
        let r = config.repos.get(repo.id()).unwrap().as_ebuild().unwrap();
        assert!(r.masters().is_empty());
        let trees: Vec<_> = r.trees().iter().map(|r| r.id().to_string()).collect();
        assert_eq!(trees, ["a"]);

        // single
        let t = TempRepo::new("b", None, None).unwrap();
        let mut repo = Repo::from_path("b", 0, t.path).unwrap();
        repo.config.set("masters", "a");
        repo.config.write(None).unwrap();
        config
            .add_repo_path(repo.id(), 0, repo.path().as_str())
            .unwrap();
        let r = config.repos.get(repo.id()).unwrap().as_ebuild().unwrap();
        let masters: Vec<_> = r.masters().iter().map(|r| r.id().to_string()).collect();
        assert_eq!(masters, ["a"]);
        let trees: Vec<_> = r.trees().iter().map(|r| r.id().to_string()).collect();
        assert_eq!(trees, ["a", "b"]);

        // multiple
        let t = TempRepo::new("c", None, None).unwrap();
        let mut repo = Repo::from_path("c", 0, t.path).unwrap();
        repo.config.set("masters", "a b");
        repo.config.write(None).unwrap();
        config
            .add_repo_path(repo.id(), 0, repo.path().as_str())
            .unwrap();
        let r = config.repos.get(repo.id()).unwrap().as_ebuild().unwrap();
        let masters: Vec<_> = r.masters().iter().map(|r| r.id().to_string()).collect();
        assert_eq!(masters, ["a", "b"]);
        let trees: Vec<_> = r.trees().into_iter().map(|r| r.id().to_string()).collect();
        assert_eq!(trees, ["a", "b", "c"]);
    }

    #[test]
    fn test_invalid_config() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (_t, repo) = config.temp_repo("test", 0).unwrap();

        repo.config.write(Some("data")).unwrap();
        let r = Repo::from_path(repo.id(), 0, repo.path());
        assert_err_re!(r, format!("^.* invalid repo config: .*$"));
    }

    #[test]
    fn test_id() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (_t, repo) = config.temp_repo("test", 0).unwrap();
        assert_eq!(repo.id(), "test");
    }

    #[test]
    fn test_len() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        assert_eq!(repo.len(), 0);
        assert!(repo.is_empty());
        t.create_ebuild("cat/pkg-1", []).unwrap();
        assert_eq!(repo.len(), 1);
        assert!(!repo.is_empty());
        t.create_ebuild("cat2/pkg-1", []).unwrap();
        assert_eq!(repo.len(), 2);
        assert!(!repo.is_empty());
    }

    #[test]
    fn test_categories() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (_t, repo) = config.temp_repo("test", 0).unwrap();

        assert!(repo.categories().is_empty());
        fs::create_dir(repo.path().join("cat")).unwrap();
        assert_eq!(repo.categories(), ["cat"]);
        fs::create_dir(repo.path().join("a-cat")).unwrap();
        fs::create_dir(repo.path().join("z-cat")).unwrap();
        assert_eq!(repo.categories(), ["a-cat", "cat", "z-cat"]);
    }

    #[test]
    fn test_packages() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (_t, repo) = config.temp_repo("test", 0).unwrap();

        assert!(repo.packages("cat").is_empty());
        fs::create_dir_all(repo.path().join("cat/pkg")).unwrap();
        assert_eq!(repo.packages("cat"), ["pkg"]);
        fs::create_dir_all(repo.path().join("a-cat/pkg-z")).unwrap();
        fs::create_dir_all(repo.path().join("a-cat/pkg-a")).unwrap();
        assert_eq!(repo.packages("a-cat"), ["pkg-a", "pkg-z"]);
    }

    #[test]
    fn test_versions() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (_t, repo) = config.temp_repo("test", 0).unwrap();

        assert!(repo.versions("cat", "pkg").is_empty());
        fs::create_dir_all(repo.path().join("cat/pkg")).unwrap();
        fs::File::create(repo.path().join("cat/pkg/pkg-1.ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), ["1"]);

        // unmatching ebuilds are ignored
        fs::File::create(repo.path().join("cat/pkg/foo-2.ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), ["1"]);

        // wrongly named files are ignored
        fs::File::create(repo.path().join("cat/pkg/pkg-2.txt")).unwrap();
        fs::File::create(repo.path().join("cat/pkg/pkg-2..ebuild")).unwrap();
        fs::File::create(repo.path().join("cat/pkg/pkg-2ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), ["1"]);

        fs::File::create(repo.path().join("cat/pkg/pkg-2.ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), ["1", "2"]);

        fs::create_dir_all(repo.path().join("a-cat/pkg10a")).unwrap();
        fs::File::create(repo.path().join("a-cat/pkg10a/pkg10a-0-r0.ebuild")).unwrap();
        assert_eq!(repo.versions("a-cat", "pkg10a"), ["0-r0"]);
    }

    #[test]
    fn test_contains() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // path containment
        assert!(!repo.contains("cat/pkg"));
        t.create_ebuild("cat/pkg-1", []).unwrap();
        assert!(repo.contains("cat/pkg"));
        assert!(repo.contains("cat/pkg/pkg-1.ebuild"));
        assert!(!repo.contains("pkg-1.ebuild"));

        // cpv containment
        let cpv = atom::cpv("cat/pkg-1").unwrap();
        assert!(repo.contains(&cpv));
        assert!(repo.contains(cpv));
        let cpv = atom::cpv("cat/pkg-2").unwrap();
        assert!(!repo.contains(&cpv));
        assert!(!repo.contains(cpv));

        // atom containment
        let a = atom::Atom::from_str("cat/pkg").unwrap();
        assert!(repo.contains(&a));
        assert!(repo.contains(a));
        let a = atom::Atom::from_str("cat/pkg-a").unwrap();
        assert!(!repo.contains(&a));
        assert!(!repo.contains(a));
    }

    #[test]
    fn test_arches() {
        // empty
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (_t, repo) = config.temp_repo("test", 0).unwrap();
        assert!(repo.arches().is_empty());

        // multiple
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (_t, repo) = config.temp_repo("test", 0).unwrap();
        let data = indoc::indoc! {r#"
            amd64
            arm64
            amd64-linux
        "#};
        fs::write(repo.profiles_base.join("arch.list"), data).unwrap();
        assert!(eq_sorted(repo.arches(), ["amd64", "arm64", "amd64-linux"]));
    }

    #[test]
    fn test_config() {
        // empty
        let t = TempRepo::new("test", None, None).unwrap();
        let repo = Repo::from_path("c", 0, t.path).unwrap();
        assert!(repo.config().properties_allowed().is_empty());
        assert!(repo.config().restrict_allowed().is_empty());

        // existing
        let t = TempRepo::new("test", None, None).unwrap();
        let mut repo = Repo::from_path("c", 0, t.path).unwrap();
        repo.config.set("properties-allowed", "interactive live");
        repo.config.set("restrict-allowed", "fetch mirror");
        repo.config.write(None).unwrap();
        assert!(eq_sorted(repo.config().properties_allowed(), ["live", "interactive"]));
        assert!(eq_sorted(repo.config().restrict_allowed(), ["fetch", "mirror"]));
    }

    #[test]
    fn test_iter() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        t.create_ebuild("cat2/pkg-1", []).unwrap();
        t.create_ebuild("cat1/pkg-1", []).unwrap();
        let mut iter = repo.iter();
        for cpv in ["cat1/pkg-1", "cat2/pkg-1"] {
            let pkg = iter.next();
            assert_eq!(pkg.map(|p| format!("{}", p.atom())), Some(cpv.to_string()));
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_iter_restrict() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        t.create_ebuild("cat/pkg-1", []).unwrap();
        t.create_ebuild("cat/pkg-2", []).unwrap();

        // single match via CPV
        let cpv = atom::cpv("cat/pkg-1").unwrap();
        let iter = repo.iter_restrict(&cpv);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, [cpv.to_string()]);

        // single match via package
        let pkg = repo.iter().next().unwrap();
        let iter = repo.iter_restrict(&pkg);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, [pkg.atom().to_string()]);

        // multiple matches
        let restrict = atom::Restrict::package("pkg");
        let iter = repo.iter_restrict(restrict);
        let atoms: Vec<_> = iter.map(|p| p.atom().to_string()).collect();
        assert_eq!(atoms, ["cat/pkg-1", "cat/pkg-2"]);
    }

    #[traced_test]
    #[test]
    fn test_invalid_pkgs() {
        for (data, err) in [
            ([(Key::Eapi, "-1")], "invalid EAPI: -1"),
            ([(Key::Eapi, "a")], "unknown EAPI: a"),
            ([(Key::Slot, "-")], "missing required values: SLOT"),
        ] {
            let mut config = Config::new("pkgcraft", "", false).unwrap();
            let (t, repo) = config.temp_repo("test", 0).unwrap();
            t.create_ebuild("cat/pkg-0", data).unwrap();
            let mut iter = repo.iter();
            assert!(iter.next().is_none());
            assert_logs_re!(format!("test repo: invalid pkg: .+: {err}$"));
        }
    }
}
