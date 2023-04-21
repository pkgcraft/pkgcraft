use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::str::{FromStr, SplitWhitespace};
use std::sync::{Arc, Weak};
use std::{fmt, fs, io, iter, thread};

use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use indexmap::IndexMap;
use ini::Ini;
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use tracing::warn;
use walkdir::{DirEntry, WalkDir};

use crate::config::RepoConfig;
use crate::dep::{self, Cpv, Operator, Version};
use crate::eapi::{Eapi, EAPI0};
use crate::files::{has_ext, is_dir, is_file, is_hidden, sorted_dir_list};
use crate::macros::build_from_paths;
use crate::pkg::ebuild::metadata::{Manifest, XmlMetadata};
use crate::pkg::ebuild::Pkg;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict, Restriction};
use crate::Error;

use super::{make_repo_traits, PkgRepository, Repo as BaseRepo, RepoFormat, Repository};

mod metadata;
pub use metadata::Metadata;

static EBUILD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?P<cat>[^/]+)/(?P<pkg>[^/]+)/(?P<p>[^/]+).ebuild$").unwrap());
const DEFAULT_SECTION: Option<String> = None;
static FAKE_CATEGORIES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["eclass", "profiles", "metadata", "licenses"]
        .into_iter()
        .collect()
});

pub struct IniConfig {
    path: Option<Utf8PathBuf>,
    ini: Ini,
}

impl Default for IniConfig {
    fn default() -> Self {
        Self { path: None, ini: Ini::new() }
    }
}

impl fmt::Debug for IniConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let section = self.ini.section(DEFAULT_SECTION);
        f.debug_struct("Metadata")
            .field("path", &self.path)
            .field("ini", &section)
            .finish()
    }
}

impl IniConfig {
    fn new(repo_path: &Utf8Path) -> crate::Result<Self> {
        let path = repo_path.join("metadata/layout.conf");
        match Ini::load_from_file(&path) {
            Ok(ini) => Ok(Self { path: Some(path), ini }),
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
    pub(crate) fn write(&self, data: Option<&str>) -> crate::Result<()> {
        use std::io::Write;
        if let Some(path) = &self.path {
            self.ini
                .write_to_file(path)
                .map_err(|e| Error::IO(e.to_string()))?;

            if let Some(data) = data {
                let mut f = fs::File::options()
                    .append(true)
                    .open(path)
                    .map_err(|e| Error::IO(e.to_string()))?;
                write!(f, "{data}").map_err(|e| Error::IO(e.to_string()))?;
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
    tx: Sender<Msg>,
    rx: Receiver<Arc<T>>,
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
        let (path_tx, path_rx) = bounded::<Msg>(10);
        let (meta_tx, meta_rx) = bounded::<Arc<T>>(10);
        let path = Utf8PathBuf::from(repo.path());

        let thread = thread::spawn(move || {
            let repo_path = path;
            let mut pkg_data = HashMap::<String, Arc<T>>::new();
            loop {
                match path_rx.recv() {
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
                        meta_tx.send(data).expect("failed sending shared pkg data");
                    }
                }
            }
        });

        Self {
            thread: Some(thread),
            tx: path_tx,
            rx: meta_rx,
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
        self.tx.send(Msg::Stop).unwrap();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

#[derive(Default)]
pub struct Repo {
    id: String,
    eapi: &'static Eapi,
    repo_config: RepoConfig,
    config: IniConfig,
    metadata: Metadata,
    name: String,
    masters: OnceCell<Vec<Weak<Self>>>,
    trees: OnceCell<Vec<Weak<Self>>>,
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
    pub(crate) fn from_path<S, P>(id: S, priority: i32, path: P) -> crate::Result<Self>
    where
        S: AsRef<str>,
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref();
        let profiles_base = path.join("profiles");

        let invalid_repo = |err: String| -> Error {
            Error::InvalidRepo {
                format: RepoFormat::Ebuild,
                path: Utf8PathBuf::from(path),
                err,
            }
        };

        if !profiles_base.exists() {
            return Err(invalid_repo("missing profiles dir".to_string()));
        }

        // verify repo name
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
                let err = format!("failed reading repo name: {:?}: {e}", &repo_name_path);
                return Err(invalid_repo(err));
            }
        };

        // verify repo EAPI
        let eapi = match fs::read_to_string(profiles_base.join("eapi")) {
            Ok(data) => {
                let s = data.lines().next().unwrap_or_default();
                <&Eapi>::from_str(s.trim_end())
                    .map_err(|e| invalid_repo(format!("invalid repo eapi: {e}")))?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => &*EAPI0,
            Err(e) => {
                let err = format!("failed reading repo eapi: {:?}: {e}", &repo_name_path);
                return Err(invalid_repo(err));
            }
        };

        let repo_config = RepoConfig {
            location: Utf8PathBuf::from(path),
            priority,
            ..Default::default()
        };

        let config = IniConfig::new(path).map_err(|e| invalid_repo(e.to_string()))?;

        Ok(Self {
            id: id.as_ref().to_string(),
            eapi,
            repo_config,
            config,
            metadata: Metadata::new(profiles_base),
            name,
            ..Default::default()
        })
    }

    pub(super) fn finalize(
        &self,
        existing_repos: &IndexMap<String, BaseRepo>,
        repo: Weak<Self>,
    ) -> crate::Result<()> {
        // skip finalized, stand-alone repos
        if self.masters.get().is_some() && self.trees.get().is_some() {
            return Ok(());
        }

        let mut nonexistent = vec![];
        let mut masters = vec![];

        for id in self.config.iter("masters") {
            match existing_repos.get(id).and_then(|r| r.as_ebuild()) {
                Some(r) => masters.push(Arc::downgrade(r)),
                None => nonexistent.push(id),
            }
        }

        if nonexistent.is_empty() {
            let mut trees = masters.clone();
            trees.push(repo);
            self.masters
                .set(masters)
                .unwrap_or_else(|_| panic!("masters already set: {}", self.id()));
            self.trees
                .set(trees)
                .unwrap_or_else(|_| panic!("trees already set: {}", self.id()));
            Ok(())
        } else {
            let repos = nonexistent.join(", ");
            Err(Error::InvalidRepo {
                format: RepoFormat::Ebuild,
                path: self.path().into(),
                err: format!("unconfigured repos: {repos}"),
            })
        }
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.repo_config
    }

    pub fn config(&self) -> &IniConfig {
        &self.config
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn eapi(&self) -> &'static Eapi {
        self.eapi
    }

    /// Return the list of inherited repos.
    pub fn masters(&self) -> Vec<Arc<Self>> {
        self.masters
            .get()
            .expect("finalize() uncalled")
            .iter()
            .map(|p| p.upgrade().expect("unconfigured repo"))
            .collect()
    }

    /// Return a repo's inheritance list including itself.
    pub fn trees(&self) -> Vec<Arc<Self>> {
        self.trees
            .get()
            .expect("finalize() uncalled")
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
                Some(s) => match dep::parse::category(s) {
                    Ok(_) => v.push(s.into()),
                    Err(e) => warn!("{e}: {path:?}"),
                },
                None => warn!("non-unicode path: {path:?}"),
            }
        }
        v
    }

    /// Convert an ebuild path inside the repo into a CPV.
    pub(crate) fn cpv_from_path(&self, path: &Utf8Path) -> crate::Result<Cpv> {
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
                Cpv::new(&format!("{cat}/{p}"))
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

    pub(crate) fn pkg_xml(&self, cpv: &Cpv) -> Arc<XmlMetadata> {
        self.xml_cache()
            .tx
            .send(Msg::Key(cpv.cpn()))
            .expect("failed requesting pkg xml data");
        self.xml_cache()
            .rx
            .recv()
            .expect("failed receiving pkg xml data")
    }

    pub(crate) fn pkg_manifest(&self, cpv: &Cpv) -> Arc<Manifest> {
        self.manifest_cache()
            .tx
            .send(Msg::Key(cpv.cpn()))
            .expect("failed requesting pkg manifest data");
        self.manifest_cache()
            .rx
            .recv()
            .expect("failed receiving pkg manifest data")
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (id, path) = (self.id.as_str(), self.path().as_str());
        if id == path {
            write!(f, "{id}")
        } else {
            write!(f, "{id}: {path}")
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
    type Iter<'a> = Iter<'a> where Self: 'a;
    type IterRestrict<'a> = IterRestrict<'a> where Self: 'a;

    fn categories(&self) -> Vec<String> {
        // use profiles/categories from repos, falling back to raw fs dirs
        let mut categories = HashSet::<String>::new();
        for r in self.trees() {
            categories.extend(r.metadata.categories().iter().cloned())
        }
        let mut categories: Vec<_> = categories.into_iter().collect();
        categories.sort();
        if categories.is_empty() {
            self.category_dirs()
        } else {
            categories
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
                    if let Some(err) = e.io_error() {
                        if err.kind() != io::ErrorKind::NotFound {
                            warn!("error walking {:?}: {e}", &path);
                        }
                    }
                    continue;
                }
            };
            let path = entry.path();
            match entry.file_name().to_str() {
                Some(s) => match dep::parse::package(s) {
                    Ok(_) => v.push(s.into()),
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
                    if let Some(err) = e.io_error() {
                        if err.kind() != io::ErrorKind::NotFound {
                            warn!("error walking {:?}: {e}", &path);
                        }
                    }
                    continue;
                }
            };
            let path = entry.path();
            let pn = path.parent().unwrap().file_name().unwrap().to_str();
            let pf = path.file_stem().unwrap().to_str();
            match (pn, pf) {
                (Some(pn), Some(pf)) => match pn == &pf[..pn.len()] {
                    true => match Version::new(&pf[pn.len() + 1..]) {
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

    fn iter(&self) -> Self::Iter<'_> {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict<'_> {
        let restrict = val.into();
        IterRestrict {
            iter: Iter::new(self, Some(&restrict)),
            restrict,
        }
    }
}

impl Repository for Repo {
    fn format(&self) -> RepoFormat {
        self.repo_config.format
    }

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

fn is_ebuild(e: &DirEntry) -> bool {
    is_file(e) && !is_hidden(e) && has_ext(e, "ebuild")
}

impl<'a> IntoIterator for &'a Repo {
    type Item = Pkg<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter::new(self, None)
    }
}

pub struct Iter<'a> {
    iter: Box<dyn Iterator<Item = (Utf8PathBuf, Cpv)> + 'a>,
    repo: &'a Repo,
}

impl<'a> Iter<'a> {
    fn new(repo: &'a Repo, restrict: Option<&Restrict>) -> Self {
        use DepRestrict::{Category, Package, Version};
        let mut cat_restricts = vec![];
        let mut pkg_restricts = vec![];
        let (mut cat, mut pkg, mut ver) = (None, None, None);

        // extract restrictions for package filtering
        if let Some(Restrict::And(vals)) = restrict {
            for r in vals.iter().map(Deref::deref) {
                match r {
                    Restrict::Dep(Category(r)) => {
                        cat_restricts.push(r.clone());
                        if let StrRestrict::Equal(s) = r {
                            cat = Some(s.to_string());
                        }
                    }
                    Restrict::Dep(r @ Package(x)) => {
                        pkg_restricts.push(r.clone());
                        if let StrRestrict::Equal(s) = x {
                            pkg = Some(s.to_string());
                        }
                    }
                    Restrict::Dep(r @ Version(x)) => {
                        pkg_restricts.push(r.clone());
                        if let Some(v) = x {
                            if v.op().is_none() || v.op() == Some(Operator::Equal) {
                                ver = Some(v.to_string());
                            }
                        }
                    }
                    _ => (),
                }
            }
        }

        // filter invalid ebuild paths
        let filter_path = |r: walkdir::Result<DirEntry>| -> Option<(Utf8PathBuf, Cpv)> {
            match r {
                Ok(e) => {
                    let path = e.path();
                    match <&Utf8Path>::try_from(path) {
                        Ok(p) => match repo.cpv_from_path(p) {
                            Ok(cpv) => Some((p.to_path_buf(), cpv)),
                            Err(e) => {
                                warn!("{}: {e}", repo.id());
                                None
                            }
                        },
                        Err(e) => {
                            warn!("{}: invalid unicode path: {path:?}: {e}", repo.id());
                            None
                        }
                    }
                }
                Err(e) => {
                    warn!("{}: failed walking repo: {e}", repo.id());
                    None
                }
            }
        };

        // return (path, cpv) tuples for pkgs in a category
        let category_pkgs = move |path: Utf8PathBuf| -> Vec<(Utf8PathBuf, Cpv)> {
            let mut paths: Vec<_> = WalkDir::new(path)
                .min_depth(2)
                .max_depth(2)
                .into_iter()
                .filter_entry(is_ebuild)
                .filter_map(filter_path)
                .collect();
            paths.sort_by(|(_p1, cpv1), (_p2, cpv2)| cpv1.cmp(cpv2));
            paths
        };

        Self {
            iter: match (cat, pkg, ver) {
                // specific package restriction
                (Some(cat), Some(pkg), Some(ver)) => {
                    let s = format!("{cat}/{pkg}-{ver}");
                    let cpv = Cpv::new(&s).expect("dep restrict failed");
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
                            .map(|s| repo.path().join(s))
                            .filter(|p| p.exists())
                            .flat_map(category_pkgs)
                            .filter(move |(_, cpv)| pkg_restrict.matches(cpv)),
                    )
                }
            },
            repo,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        for (path, cpv) in &mut self.iter {
            match Pkg::new(path, cpv, self.repo) {
                Ok(pkg) => return Some(pkg),
                Err(e) => warn!("{} repo: {e}", self.repo.id()),
            }
        }
        None
    }
}

pub struct IterRestrict<'a> {
    iter: Iter<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for IterRestrict<'a> {
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use tracing_test::traced_test;

    use crate::config::Config;
    use crate::dep::Dep;
    use crate::eapi::EAPI_LATEST_OFFICIAL;
    use crate::macros::*;
    use crate::pkg::Package;
    use crate::pkgsh::metadata::Key;
    use crate::repo::ebuild_temp::Repo as TempRepo;
    use crate::repo::Contains;
    use crate::test::assert_unordered_eq;

    use super::*;

    #[test]
    fn test_masters() {
        let mut config = Config::default();

        // nonexistent
        let t = TempRepo::new("test", None, None).unwrap();
        let mut repo = Repo::from_path("test", 0, t.path()).unwrap();
        repo.config.set("masters", "a b c");
        repo.config.write(None).unwrap();
        let r = config.add_repo_path(repo.id(), 0, repo.path().as_str());
        assert_err_re!(r, "^.* unconfigured repos: a, b, c$");

        // none
        let t = TempRepo::new("a", None, None).unwrap();
        let repo = Repo::from_path("a", 0, t.path()).unwrap();
        config
            .add_repo_path(repo.id(), 0, repo.path().as_str())
            .unwrap();
        let r = config.repos.get(repo.id()).unwrap().as_ebuild().unwrap();
        assert!(r.masters().is_empty());
        let trees: Vec<_> = r.trees().iter().map(|r| r.id().to_string()).collect();
        assert_eq!(trees, ["a"]);

        // single
        let t = TempRepo::new("b", None, None).unwrap();
        let mut repo = Repo::from_path("b", 0, t.path()).unwrap();
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
        let mut repo = Repo::from_path("c", 0, t.path()).unwrap();
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
        let mut config = Config::default();
        let (_t, repo) = config.temp_repo("test", 0).unwrap();

        repo.config.write(Some("data")).unwrap();
        let r = Repo::from_path(repo.id(), 0, repo.path());
        assert_err_re!(r, "^.* invalid repo config: .*$");
    }

    #[test]
    fn test_id_and_name() {
        // repo id matches name
        let t = TempRepo::new("test", None, None).unwrap();
        let repo = Repo::from_path("test", 0, t.path()).unwrap();
        assert_eq!(repo.id(), "test");
        assert_eq!(repo.name(), "test");

        // repo id differs from name
        let t = TempRepo::new("name", None, None).unwrap();
        let repo = Repo::from_path("id", 0, t.path()).unwrap();
        assert_eq!(repo.id(), "id");
        assert_eq!(repo.name(), "name");
    }

    #[test]
    fn test_eapi() {
        // repos lacking profiles/eapi file use EAPI0
        let t = TempRepo::new("test", None, None).unwrap();
        let repo = Repo::from_path("test", 0, t.path()).unwrap();
        assert_eq!(repo.eapi(), &*EAPI0);

        // explicit repo EAPI
        let t = TempRepo::new("test", None, Some(*EAPI_LATEST_OFFICIAL)).unwrap();
        let repo = Repo::from_path("test", 0, t.path()).unwrap();
        assert_eq!(repo.eapi(), *EAPI_LATEST_OFFICIAL);
    }

    #[test]
    fn test_len() {
        let mut config = Config::default();
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
        let mut config = Config::default();
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
        let mut config = Config::default();
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
        let mut config = Config::default();
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
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // path
        assert!(!repo.contains("cat/pkg"));
        t.create_ebuild("cat/pkg-1", []).unwrap();
        assert!(repo.contains("cat/pkg"));
        assert!(repo.contains("cat/pkg/pkg-1.ebuild"));
        assert!(!repo.contains("pkg-1.ebuild"));

        // cpv
        let cpv = Cpv::new("cat/pkg-1").unwrap();
        assert!(repo.contains(&cpv));
        let cpv = Cpv::new("cat/pkg-2").unwrap();
        assert!(!repo.contains(&cpv));

        // unversioned dep
        let d = Dep::from_str("cat/pkg").unwrap();
        assert!(repo.contains(&d));
        let d = Dep::from_str("cat/pkg-a").unwrap();
        assert!(!repo.contains(&d));
    }

    #[test]
    fn test_config() {
        // empty
        let t = TempRepo::new("test", None, None).unwrap();
        let repo = Repo::from_path("c", 0, t.path()).unwrap();
        assert!(repo.config().properties_allowed().is_empty());
        assert!(repo.config().restrict_allowed().is_empty());

        // existing
        let t = TempRepo::new("test", None, None).unwrap();
        let mut repo = Repo::from_path("c", 0, t.path()).unwrap();
        repo.config.set("properties-allowed", "interactive live");
        repo.config.set("restrict-allowed", "fetch mirror");
        repo.config.write(None).unwrap();
        assert_unordered_eq(repo.config().properties_allowed(), ["live", "interactive"]);
        assert_unordered_eq(repo.config().restrict_allowed(), ["fetch", "mirror"]);
    }

    #[test]
    fn test_iter() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        t.create_ebuild("cat2/pkg-1", []).unwrap();
        t.create_ebuild("cat1/pkg-1", []).unwrap();
        let mut iter = repo.iter();
        for cpv in ["cat1/pkg-1", "cat2/pkg-1"] {
            let pkg = iter.next();
            assert_eq!(pkg.map(|p| format!("{}", p.cpv())), Some(cpv.to_string()));
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_iter_restrict() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        t.create_ebuild("cat/pkg-1", []).unwrap();
        t.create_ebuild("cat/pkg-2", []).unwrap();

        // single match via CPV
        let cpv = Cpv::new("cat/pkg-1").unwrap();
        let iter = repo.iter_restrict(&cpv);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, [cpv.to_string()]);

        // single match via package
        let pkg = repo.iter().next().unwrap();
        let iter = repo.iter_restrict(&pkg);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, [pkg.cpv().to_string()]);

        // multiple matches
        let restrict = DepRestrict::package("pkg");
        let iter = repo.iter_restrict(restrict);
        let cpvs: Vec<_> = iter.map(|p| p.cpv().to_string()).collect();
        assert_eq!(cpvs, ["cat/pkg-1", "cat/pkg-2"]);
    }

    #[traced_test]
    #[test]
    fn test_invalid_pkgs() {
        for (data, err) in [
            ([(Key::Eapi, "-1")], "invalid EAPI: -1"),
            ([(Key::Eapi, "a")], "unknown EAPI: a"),
            ([(Key::Slot, "-")], "missing required values: SLOT"),
        ] {
            let mut config = Config::default();
            let (t, repo) = config.temp_repo("test", 0).unwrap();
            t.create_ebuild("cat/pkg-0", data).unwrap();
            let mut iter = repo.iter();
            assert!(iter.next().is_none());
            assert_logs_re!(format!("test repo: invalid pkg: .+: {err}$"));
        }
    }
}
