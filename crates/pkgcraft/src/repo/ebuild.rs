use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::{fmt, fs, io, iter, thread};

use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use indexmap::{Equivalent, IndexMap, IndexSet};
use itertools::{Either, Itertools};
use once_cell::sync::{Lazy, OnceCell};
use scallop::pool::PoolIter;
use tracing::{error, warn};
use walkdir::{DirEntry, WalkDir};

use crate::config::RepoConfig;
use crate::dep::{self, Cpv, Operator, Version};
use crate::eapi::Eapi;
use crate::files::{has_ext, is_dir, is_file, is_hidden, sorted_dir_list};
use crate::macros::build_from_paths;
use crate::pkg::ebuild::metadata::{Manifest, XmlMetadata};
use crate::pkg::ebuild::{Pkg, RawPkg};
use crate::pkg::SourceablePackage;
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict, Restriction};
use crate::utils::digest;
use crate::Error;

use super::{make_repo_traits, PkgRepository, Repo as BaseRepo, RepoFormat, Repository};

mod metadata;
pub use metadata::Metadata;

// root level directories that aren't categories
static FAKE_CATEGORIES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["eclass", "profiles", "metadata", "licenses"]
        .into_iter()
        .collect()
});

/// Shared data cache trait.
pub(crate) trait CacheData: Default {
    const RELPATH: &'static str;
    fn parse(data: &str) -> crate::Result<Self>;
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
    fn new(repo: Arc<Repo>) -> Cache<T> {
        let (path_tx, path_rx) = bounded::<Msg>(10);
        let (meta_tx, meta_rx) = bounded::<Arc<T>>(10);

        let thread = thread::spawn(move || {
            // TODO: limit cache size using an LRU cache with set capacity
            let mut pkg_cache = HashMap::<String, (blake3::Hash, Arc<T>)>::new();
            loop {
                match path_rx.recv() {
                    Ok(Msg::Stop) | Err(RecvError) => break,
                    Ok(Msg::Key(s)) => {
                        let path = build_from_paths!(repo.path(), &s, T::RELPATH);
                        let data = fs::read_to_string(&path)
                            .map_err(|e| {
                                if e.kind() != io::ErrorKind::NotFound {
                                    warn!("{}: failed reading: {path}: {e}", repo.id());
                                }
                            })
                            .unwrap_or_default();

                        // evict cache entries based on file content hash
                        let hash = blake3::hash(data.as_bytes());

                        let val = match pkg_cache.get(&s) {
                            Some((cached_hash, val)) if cached_hash == &hash => val.clone(),
                            _ => {
                                // fallback to default value on parsing failure
                                let val = T::parse(&data)
                                    .map_err(|e| {
                                        warn!("{}: failed parsing: {path}: {e}", repo.id());
                                    })
                                    .unwrap_or_default();

                                // insert Arc-wrapped value into the cache and return a copy
                                let val = Arc::new(val);
                                pkg_cache.insert(s, (hash, val.clone()));
                                val
                            }
                        };

                        meta_tx.send(val).expect("failed sending shared pkg data");
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

    /// Get the cache data related to a given package Cpv.
    fn get(&self, cpv: &Cpv) -> Arc<T> {
        self.tx
            .send(Msg::Key(cpv.cpn()))
            .expect("failed requesting pkg manifest data");
        self.rx.recv().expect("failed receiving pkg manifest data")
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

#[derive(Debug)]
pub struct Eclass {
    name: String,
    path: Utf8PathBuf,
    digest: String,
}

impl Eclass {
    fn new(path: &Utf8Path) -> crate::Result<Self> {
        if let (Some(name), Some("eclass")) = (path.file_stem(), path.extension()) {
            let data = fs::read(path)
                .map_err(|e| Error::IO(format!("failed reading eclass: {path}: {e}")))?;

            Ok(Self {
                name: name.to_string(),
                path: path.to_path_buf(),
                digest: digest::<md5::Md5>(&data),
            })
        } else {
            Err(Error::InvalidValue(format!("invalid eclass: {path}")))
        }
    }

    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl fmt::Display for Eclass {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl AsRef<str> for Eclass {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl Eq for Eclass {}

impl PartialEq for Eclass {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Ord for Eclass {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Eclass {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for Eclass {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Borrow<str> for Eclass {
    fn borrow(&self) -> &str {
        &self.name
    }
}

impl Equivalent<String> for Eclass {
    fn equivalent(&self, key: &String) -> bool {
        &self.name == key
    }
}

#[derive(Default)]
pub struct Repo {
    id: String,
    config: RepoConfig,
    metadata: Metadata,
    masters: OnceCell<Vec<Weak<Self>>>,
    trees: OnceCell<Vec<Weak<Self>>>,
    arches: OnceCell<HashSet<String>>,
    licenses: OnceCell<HashSet<String>>,
    license_groups: OnceCell<HashMap<String, HashSet<String>>>,
    mirrors: OnceCell<IndexMap<String, IndexSet<String>>>,
    eclasses: OnceCell<HashSet<Eclass>>,
    xml_cache: OnceCell<Cache<XmlMetadata>>,
    manifest_cache: OnceCell<Cache<Manifest>>,
}

impl fmt::Debug for Repo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Repo")
            .field("id", &self.id)
            .field("repo_config", &self.repo_config())
            .field("name", &self.name())
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

        let config = RepoConfig {
            location: Utf8PathBuf::from(path),
            priority,
            ..Default::default()
        };

        Ok(Self {
            config,
            metadata: Metadata::new(id.as_ref(), path)?,
            ..Default::default()
        })
    }

    /// Finalize the repo, collapsing repo dependencies into references.
    pub(super) fn finalize(
        &self,
        existing_repos: &IndexMap<String, BaseRepo>,
        repo: Weak<Self>,
    ) -> crate::Result<()> {
        // skip finalized, stand-alone repos
        if self.masters.get().is_some() && self.trees.get().is_some() {
            return Ok(());
        }

        let repo_ids = self.metadata().config().masters().iter();
        let (masters, nonexistent): (Vec<_>, Vec<_>) =
            repo_ids.partition_map(|id| match existing_repos.get(id).and_then(|r| r.as_ebuild()) {
                Some(r) => Either::Left(Arc::downgrade(r)),
                None => Either::Right(id.as_str()),
            });

        if nonexistent.is_empty() {
            let mut trees = masters.clone();
            trees.push(repo);
            self.masters
                .set(masters)
                .unwrap_or_else(|_| panic!("masters already set: {}", self.id()));
            self.trees
                .set(trees)
                .unwrap_or_else(|_| panic!("trees already set: {}", self.id()));
            self.collapse_lazy_fields();
            Ok(())
        } else {
            let repos = nonexistent.join(", ");
            Err(Error::InvalidRepo {
                id: self.id().to_string(),
                err: format!("unconfigured repos: {repos}"),
            })
        }
    }

    /// Collapse various lazy fields that require repo dependencies.
    ///
    /// This is called during repo finalization when not running tests in order to avoid duplicate
    /// calls when run under forked processes such as during package cache generation.
    fn collapse_lazy_fields(&self) {
        if !cfg!(any(test, feature = "test")) {
            self.eclasses();
        }
    }

    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.config
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn eapi(&self) -> &'static Eapi {
        self.metadata().eapi
    }

    /// Return the inherited repos for the repo.
    pub fn masters(&self) -> impl Iterator<Item = Arc<Self>> + '_ {
        self.masters
            .get()
            .expect("finalize() uncalled")
            .iter()
            .map(|p| p.upgrade().expect("unconfigured repo"))
    }

    /// Return the complete, repo inheritance set for the repo.
    pub fn trees(&self) -> impl Iterator<Item = Arc<Self>> + '_ {
        self.trees
            .get()
            .expect("finalize() uncalled")
            .iter()
            .map(|p| p.upgrade().expect("unconfigured repo"))
    }

    /// Return an Arc-wrapped repo reference.
    fn arc(&self) -> Arc<Self> {
        self.trees
            .get()
            .expect("finalize() uncalled")
            .last()
            .map(|p| p.upgrade().expect("unconfigured repo"))
            .expect("finalize() uncalled")
    }

    /// Return the mapping of inherited eclass names to file paths.
    pub fn eclasses(&self) -> &HashSet<Eclass> {
        self.eclasses.get_or_init(|| {
            self.trees()
                .filter_map(|repo| repo.path().join("eclass").read_dir_utf8().ok())
                .flatten()
                .filter_map(|e| e.ok())
                .filter_map(|e| Eclass::new(e.path()).ok())
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
                    warn!("{}: failed walking {:?}: {e}", self.id(), self.path());
                    continue;
                }
            };
            let path = entry.path();
            match entry.file_name().to_str() {
                Some(s) => match dep::parse::category(s) {
                    Ok(_) => v.push(s.into()),
                    Err(e) => warn!("{}: {e}: {path:?}", self.id()),
                },
                None => warn!("{}: non-unicode path: {path:?}", self.id()),
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
            .and_then(|path| {
                let (cat, pkg, file) = path
                    .components()
                    .map(|s| s.as_str())
                    .collect_tuple()
                    .ok_or_else(|| err("mismatched path components"))?;
                let p = file
                    .strip_suffix(".ebuild")
                    .ok_or_else(|| err("missing ebuild ext"))?;
                Cpv::new(&format!("{cat}/{p}"))
                    .map_err(|_| err("invalid CPV"))
                    .and_then(|a| {
                        if a.package() == pkg {
                            Ok(a)
                        } else {
                            Err(err("mismatched package dir"))
                        }
                    })
            })
    }

    /// Return the set of known architectures merged via inheritance.
    pub fn arches(&self) -> &HashSet<String> {
        self.arches.get_or_init(|| {
            self.trees()
                .flat_map(|r| r.metadata().arches().clone().into_iter())
                .collect()
        })
    }

    /// Return the set of licenses merged via inheritance.
    pub fn licenses(&self) -> &HashSet<String> {
        self.licenses.get_or_init(|| {
            self.trees()
                .flat_map(|r| r.metadata().licenses().clone().into_iter())
                .collect()
        })
    }

    /// Return the mapping of license groups merged via inheritance.
    pub fn license_groups(&self) -> &HashMap<String, HashSet<String>> {
        self.license_groups.get_or_init(|| {
            let mut group_map = self.metadata().license_groups().clone();
            self.masters()
                .flat_map(|r| r.metadata().license_groups().clone().into_iter())
                .for_each(|(name, set)| {
                    group_map
                        .entry(name)
                        .or_insert_with(HashSet::new)
                        .extend(set);
                });
            group_map
        })
    }

    /// Return the set of mirrors merged via inheritance.
    pub fn mirrors(&self) -> &IndexMap<String, IndexSet<String>> {
        self.mirrors.get_or_init(|| {
            self.trees()
                .flat_map(|r| r.metadata().mirrors().clone().into_iter())
                .collect()
        })
    }

    /// Return the shared XML metadata for a given package.
    pub(crate) fn pkg_xml(&self, cpv: &Cpv) -> Arc<XmlMetadata> {
        self.xml_cache
            .get_or_init(|| Cache::<XmlMetadata>::new(self.arc()))
            .get(cpv)
    }

    /// Return the shared manifest data for a given package.
    pub(crate) fn pkg_manifest(&self, cpv: &Cpv) -> Arc<Manifest> {
        self.manifest_cache
            .get_or_init(|| Cache::<Manifest>::new(self.arc()))
            .get(cpv)
    }

    /// Regenerate the package metadata cache, returning the number of errors that occurred.
    pub fn pkg_metadata_regen<F: Fn()>(
        &self,
        jobs: usize,
        force: bool,
        callback: Option<F>,
    ) -> crate::Result<usize> {
        let pkgs = self.iter_raw();
        let func = |pkg: RawPkg| pkg.metadata(force);
        let mut errors = 0;

        for r in PoolIter::new(jobs, pkgs, func)? {
            // log errors
            if let Err(e) = r {
                errors += 1;
                error!("{e}");
            }

            // run callback per result to support features such as progress indication
            if let Some(cb) = &callback {
                cb();
            }
        }

        Ok(errors)
    }

    /// Return an iterator of raw packages for the repo.
    pub fn iter_raw(&self) -> IterRaw<'_> {
        IterRaw::new(self, None)
    }

    /// Return a filtered iterator of raw packages for the repo.
    pub fn iter_raw_restrict<R: Into<Restrict>>(&self, val: R) -> IterRawRestrict<'_> {
        let restrict = val.into();
        IterRawRestrict {
            iter: IterRaw::new(self, Some(&restrict)),
            restrict,
        }
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
            categories.extend(r.metadata().categories().iter().cloned())
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
                            warn!("{}: failed walking {:?}: {e}", self.id(), &path);
                        }
                    }
                    continue;
                }
            };
            let path = entry.path();
            match entry.file_name().to_str() {
                Some(s) => match dep::parse::package(s) {
                    Ok(_) => v.push(s.into()),
                    Err(e) => warn!("{}: {e}: {path:?}", self.id()),
                },
                None => warn!("{}: non-unicode path: {path:?}", self.id()),
            }
        }
        v
    }

    fn versions(&self, cat: &str, pkg: &str) -> Vec<Version> {
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
                            warn!("{}: failed walking {:?}: {e}", self.id(), &path);
                        }
                    }
                    continue;
                }
            };
            let path = entry.path();
            let pn = path.parent().unwrap().file_name().unwrap().to_str();
            let pf = path.file_stem().unwrap().to_str();
            if let (Some(pn), Some(pf)) = (pn, pf) {
                if pn == &pf[..pn.len()] {
                    match Version::new(&pf[pn.len() + 1..]) {
                        Ok(v) => versions.push(v),
                        Err(e) => warn!("{}: {e}: {path:?}", self.id()),
                    }
                } else {
                    warn!("{}: unmatched ebuild: {path:?}", self.id());
                }
            } else {
                warn!("{}: non-unicode path: {path:?}", self.id());
            }
        }
        versions.sort();
        versions
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
        self.repo_config().format
    }

    fn id(&self) -> &str {
        &self.metadata().id
    }

    fn name(&self) -> &str {
        &self.metadata().name
    }

    fn priority(&self) -> i32 {
        self.repo_config().priority
    }

    fn path(&self) -> &Utf8Path {
        &self.repo_config().location
    }

    fn sync(&self) -> crate::Result<()> {
        self.repo_config().sync()
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
    iter: IterRaw<'a>,
    repo: &'a Repo,
}

impl<'a> Iter<'a> {
    fn new(repo: &'a Repo, restrict: Option<&Restrict>) -> Self {
        let iter = IterRaw::new(repo, restrict);
        Self { iter, repo }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        for raw_pkg in &mut self.iter {
            match raw_pkg.into_pkg() {
                Ok(pkg) => return Some(pkg),
                Err(e) => warn!("{}: {e}", self.repo.id()),
            }
        }
        None
    }
}

pub struct IterRaw<'a> {
    iter: Box<dyn Iterator<Item = (Utf8PathBuf, Cpv)> + 'a>,
    repo: &'a Repo,
}

impl<'a> IterRaw<'a> {
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
                                ver = Some(v.as_str().to_string());
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

impl<'a> Iterator for IterRaw<'a> {
    type Item = RawPkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        for (path, cpv) in &mut self.iter {
            match RawPkg::new(path, cpv, self.repo) {
                Ok(pkg) => return Some(pkg),
                Err(e) => warn!("{}: {e}", self.repo.id()),
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

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|pkg| self.restrict.matches(pkg))
    }
}

pub struct IterRawRestrict<'a> {
    iter: IterRaw<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for IterRawRestrict<'a> {
    type Item = RawPkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|pkg| self.restrict.matches(pkg))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::str::FromStr;

    use tracing_test::traced_test;

    use crate::config::Config;
    use crate::dep::Dep;
    use crate::eapi::{EAPI0, EAPI_LATEST_OFFICIAL};
    use crate::macros::*;
    use crate::pkg::Package;
    use crate::repo::ebuild_temp::Repo as TempRepo;
    use crate::repo::Contains;
    use crate::test::{assert_unordered_eq, TEST_DATA};

    use super::*;

    #[test]
    fn test_masters() {
        let mut config = Config::default();
        let repos_path = TEST_DATA.path.join("repos");

        // none
        let repo = Repo::from_path("a", 0, repos_path.join("dependent-primary")).unwrap();
        let repo = config
            .add_repo_path(repo.id(), 0, repo.path().as_str())
            .unwrap();
        let repo = repo.as_ebuild().unwrap();
        assert!(repo.masters().next().is_none());
        let trees: Vec<_> = repo.trees().map(|r| r.id().to_string()).collect();
        assert_eq!(trees, ["a"]);

        // nonexistent
        let repo = Repo::from_path("test", 0, repos_path.join("dependent-nonexistent")).unwrap();
        let r = config.add_repo_path(repo.id(), 0, repo.path().as_str());
        assert_err_re!(r, "^.* unconfigured repos: nonexistent1, nonexistent2$");

        // single
        let repo = Repo::from_path("b", 0, repos_path.join("dependent-secondary")).unwrap();
        let repo = config
            .add_repo_path(repo.id(), 0, repo.path().as_str())
            .unwrap();
        let repo = repo.as_ebuild().unwrap();
        let masters: Vec<_> = repo.masters().map(|r| r.id().to_string()).collect();
        assert_eq!(masters, ["a"]);
        let trees: Vec<_> = repo.trees().map(|r| r.id().to_string()).collect();
        assert_eq!(trees, ["a", "b"]);
    }

    #[test]
    fn test_id_and_name() {
        // repo id matches name
        let t = TempRepo::new("test", None, 0, None).unwrap();
        assert_eq!(t.repo().id(), "test");
        assert_eq!(t.repo().name(), "test");

        // repo id differs from name
        let t = TempRepo::new("name", None, 0, None).unwrap();
        let repo = Repo::from_path("id", 0, t.path()).unwrap();
        assert_eq!(repo.id(), "id");
        assert_eq!(repo.name(), "name");
    }

    #[test]
    fn test_eapi() {
        // repos lacking profiles/eapi file use EAPI0
        let t = TempRepo::new("test", None, 0, None).unwrap();
        assert_eq!(t.repo().eapi(), &*EAPI0);

        // explicit repo EAPI
        let t = TempRepo::new("test", None, 0, Some(*EAPI_LATEST_OFFICIAL)).unwrap();
        assert_eq!(t.repo().eapi(), *EAPI_LATEST_OFFICIAL);
    }

    #[test]
    fn test_len() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();

        assert_eq!(repo.len(), 0);
        assert!(repo.is_empty());
        t.create_ebuild("cat/pkg-1", &[]).unwrap();
        assert_eq!(repo.len(), 1);
        assert!(!repo.is_empty());
        t.create_ebuild("cat2/pkg-1", &[]).unwrap();
        assert_eq!(repo.len(), 2);
        assert!(!repo.is_empty());
    }

    #[test]
    fn test_categories() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();

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
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();

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
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();
        let ver = |s: &str| Version::new(s).unwrap();

        assert!(repo.versions("cat", "pkg").is_empty());
        fs::create_dir_all(repo.path().join("cat/pkg")).unwrap();
        fs::File::create(repo.path().join("cat/pkg/pkg-1.ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), [ver("1")]);

        // unmatching ebuilds are ignored
        fs::File::create(repo.path().join("cat/pkg/foo-2.ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), [ver("1")]);

        // wrongly named files are ignored
        fs::File::create(repo.path().join("cat/pkg/pkg-2.txt")).unwrap();
        fs::File::create(repo.path().join("cat/pkg/pkg-2..ebuild")).unwrap();
        fs::File::create(repo.path().join("cat/pkg/pkg-2ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), [ver("1")]);

        fs::File::create(repo.path().join("cat/pkg/pkg-2.ebuild")).unwrap();
        assert_eq!(repo.versions("cat", "pkg"), [ver("1"), ver("2")]);

        fs::create_dir_all(repo.path().join("a-cat/pkg10a")).unwrap();
        fs::File::create(repo.path().join("a-cat/pkg10a/pkg10a-0-r0.ebuild")).unwrap();
        assert_eq!(repo.versions("a-cat", "pkg10a"), [ver("0-r0")]);
    }

    #[test]
    fn test_contains() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();

        // path
        assert!(!repo.contains("cat/pkg"));
        t.create_ebuild("cat/pkg-1", &[]).unwrap();
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
    fn test_iter() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();
        t.create_ebuild("cat2/pkg-1", &[]).unwrap();
        t.create_ebuild("cat1/pkg-1", &[]).unwrap();
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
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();
        t.create_ebuild("cat/pkg-1", &[]).unwrap();
        t.create_ebuild("cat/pkg-2", &[]).unwrap();

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
            ("EAPI=-1", "invalid EAPI: \"-1\""),
            ("EAPI=a", "unknown EAPI: a"),
            ("SLOT=", "missing required values: SLOT"),
        ] {
            let mut config = Config::default();
            let t = config.temp_repo("test", 0, None).unwrap();
            t.create_ebuild("cat/pkg-0", &[data]).ok();
            let mut iter = t.repo().iter();
            assert!(iter.next().is_none());
            assert_logs_re!(format!("test: invalid pkg: .+: {err}$"));
        }
    }

    #[test]
    fn test_eclasses() {
        let repo = TEST_DATA.ebuild_repo("dependent-primary").unwrap();
        assert_unordered_eq(repo.eclasses().iter().map(|e| e.as_ref()), ["a"]);
        let repo = TEST_DATA.ebuild_repo("dependent-secondary").unwrap();
        assert_unordered_eq(repo.eclasses().iter().map(|e| e.as_ref()), ["a", "b"]);
    }

    #[test]
    fn test_arches() {
        let repo = TEST_DATA.ebuild_repo("dependent-primary").unwrap();
        assert_unordered_eq(repo.arches(), ["x86"]);
        let repo = TEST_DATA.ebuild_repo("dependent-secondary").unwrap();
        assert_unordered_eq(repo.arches(), ["amd64", "x86"]);
    }

    #[test]
    fn test_licenses() {
        let repo = TEST_DATA.ebuild_repo("dependent-primary").unwrap();
        assert_unordered_eq(repo.licenses(), ["a"]);
        let repo = TEST_DATA.ebuild_repo("dependent-secondary").unwrap();
        assert_unordered_eq(repo.licenses(), ["a", "b"]);
    }
}
