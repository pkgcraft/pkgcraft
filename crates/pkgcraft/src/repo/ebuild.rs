use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::path::Path;
use std::sync::{Arc, OnceLock, Weak};
use std::{fmt, fs, io, iter, thread};

use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use indexmap::{Equivalent, IndexMap, IndexSet};
use indicatif::ProgressBar;
use itertools::{Either, Itertools};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use scallop::pool::PoolSendIter;
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
use crate::shell::metadata::Metadata as MetadataCache;
use crate::utils::digest;
use crate::Error;

use super::{make_repo_traits, Contains, PkgRepository, Repo as BaseRepo, RepoFormat, Repository};

mod cache;
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
// Cache is contained in a OnceLock that doesn't call drop().
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
    masters: OnceLock<Vec<Weak<Self>>>,
    trees: OnceLock<Vec<Weak<Self>>>,
    arches: OnceLock<HashSet<String>>,
    licenses: OnceLock<HashSet<String>>,
    license_groups: OnceLock<HashMap<String, HashSet<String>>>,
    mirrors: OnceLock<IndexMap<String, IndexSet<String>>>,
    eclasses: OnceLock<HashSet<Eclass>>,
    xml_cache: OnceLock<Cache<XmlMetadata>>,
    manifest_cache: OnceLock<Cache<Manifest>>,
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
        if !*crate::test::TESTING {
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
    pub fn category_dirs(&self) -> IndexSet<String> {
        // filter out non-category dirs
        let filter = |e: &DirEntry| -> bool { is_dir(e) && !is_hidden(e) && !is_fake_category(e) };
        let cats = sorted_dir_list(self.path())
            .into_iter()
            .filter_entry(filter);
        let mut v = IndexSet::new();
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
                    Ok(_) => {
                        v.insert(s.into());
                    }
                    Err(e) => warn!("{}: {e}: {path:?}", self.id()),
                },
                None => warn!("{}: non-unicode path: {path:?}", self.id()),
            }
        }
        v
    }

    /// Convert a relative ebuild file repo path into a CPV.
    fn cpv_from_ebuild_path<P: AsRef<Path>>(&self, path: P) -> crate::Result<Cpv> {
        let path = path.as_ref();
        let err = |s: &str| -> Error {
            Error::InvalidValue(format!("invalid ebuild path: {path:?}: {s}"))
        };

        let relpath = path
            .strip_prefix(self.path())
            .map_err(|_| err("missing repo prefix"))?;
        let relpath = <&Utf8Path>::try_from(relpath).map_err(|_| err("invalid unicode path"))?;

        let (cat, pkg, file) = relpath
            .components()
            .map(|s| s.as_str())
            .collect_tuple()
            .ok_or_else(|| err("mismatched path components"))?;
        let p = file
            .strip_suffix(".ebuild")
            .ok_or_else(|| err("missing ebuild ext"))?;
        Cpv::new(format!("{cat}/{p}"))
            .map_err(|_| err("invalid CPV"))
            .and_then(|a| {
                if a.package() == pkg {
                    Ok(a)
                } else {
                    Err(err("mismatched package dir"))
                }
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

    /// Return the sorted set of Cpvs in a given category.
    fn category_cpvs(&self, category: &str) -> IndexSet<Cpv> {
        // filter invalid ebuild paths
        let filter_path = |r: walkdir::Result<DirEntry>| -> Option<Cpv> {
            match r {
                Ok(e) => match self.cpv_from_ebuild_path(e.path()) {
                    Ok(cpv) => Some(cpv),
                    Err(e) => {
                        warn!("{}: {e}", self.id());
                        None
                    }
                },
                Err(e) => {
                    warn!("{}: failed walking repo: {e}", self.id());
                    None
                }
            }
        };

        let mut cpvs: IndexSet<_> = WalkDir::new(self.path().join(category))
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .filter_entry(is_ebuild)
            .filter_map(filter_path)
            .collect();
        cpvs.sort();
        cpvs
    }

    /// Regenerate the package metadata cache, returning the number of errors that occurred.
    pub fn pkg_metadata_regen(
        &self,
        jobs: usize,
        force: bool,
        progress: bool,
    ) -> crate::Result<usize> {
        // initialize pool first to minimize forked process memory pages
        let func = |cpv: Cpv| {
            let pkg = RawPkg::new(cpv, self)?;
            pkg.metadata()
        };
        let pool = PoolSendIter::new(jobs, func, true)?;

        // TODO: replace with parallel Cpv iterator -- repo.par_iter_cpvs()
        // pull all package Cpvs from the repo
        let mut cpvs: HashSet<_> = self
            .categories()
            .into_par_iter()
            .flat_map(|s| self.category_cpvs(&s))
            .collect();

        // use progress bar to show completion progress when outputting to a terminal
        let pb = ProgressBar::new(cpvs.len().try_into().unwrap());

        let path = self.metadata().cache_path();
        if path.exists() {
            // remove outdated cache entries lacking matching ebuilds
            WalkDir::new(path)
                .min_depth(2)
                .max_depth(2)
                .into_iter()
                .filter_map(|e| e.ok())
                .try_for_each(|e| {
                    e.path()
                        .strip_prefix(path)
                        .ok()
                        .and_then(|p| Cpv::new(p.to_string_lossy()).ok())
                        .filter(|cpv| !cpvs.contains(cpv))
                        .map_or(Ok(()), |_| {
                            fs::remove_file(e.path()).map_err(|e| {
                                Error::IO(format!("failed removing metadata cache entry: {e}"))
                            })
                        })
                })?;

            if !force {
                // run cache validation in a thread pool
                cpvs = cpvs
                    .into_par_iter()
                    .filter(|cpv| {
                        if progress {
                            pb.inc(1);
                        }
                        MetadataCache::load(cpv, self).is_err()
                    })
                    .collect();

                // reset progression in case validation decreased cpvs
                if progress {
                    pb.set_position(0);
                    pb.set_length(cpvs.len().try_into().unwrap());
                }
            }
        }

        // send Cpvs and iterate over returned results, tracking progress and errors
        let mut errors = 0;
        if !cpvs.is_empty() {
            for r in pool.iter(cpvs.into_iter())? {
                if progress {
                    pb.inc(1);
                }

                // log errors
                if let Err(e) = r {
                    errors += 1;
                    error!("{e}");
                }
            }
        }

        Ok(errors)
    }

    /// Return an iterator of Cpvs for the repo.
    pub fn iter_cpv(&self) -> IterCpv {
        IterCpv::new(self, None)
    }

    /// Return a filtered iterator of Cpvs for the repo.
    pub fn iter_cpv_restrict<R: Into<Restrict>>(&self, val: R) -> IterCpvRestrict<'_> {
        let restrict = val.into();
        IterCpvRestrict {
            iter: IterCpv::new(self, Some(&restrict)),
            restrict,
        }
    }

    /// Return an iterator of raw packages for the repo.
    pub fn iter_raw(&self) -> IterRaw {
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

    /// Try converting a path to a [`Restrict`], returns None if the path isn't in the repo.
    pub fn restrict_from_path<P: AsRef<Utf8Path>>(&self, path: P) -> Option<Restrict> {
        let path = path.as_ref().canonicalize_utf8().ok()?;
        if self.contains(&path) {
            let mut restricts = vec![DepRestrict::repo(Some(self.id()))];

            let relpath = path.strip_prefix(self.path()).unwrap_or(&path);
            let components: Vec<_> = relpath.components().map(|c| c.as_str()).collect();
            for (i, s) in components.iter().enumerate() {
                match (i, s) {
                    (0, s) if self.categories().contains(*s) => {
                        restricts.push(DepRestrict::category(s));
                    }
                    (1, s) if self.packages(components[0]).contains(*s) => {
                        restricts.push(DepRestrict::package(s));
                    }
                    (2, s) if s.ends_with(".ebuild") => {
                        if let Ok(cpv) = self.cpv_from_ebuild_path(&path) {
                            let ver = cpv.version().clone();
                            restricts.push(DepRestrict::Version(Some(ver)));
                        }
                    }
                    _ => (),
                }
            }

            Some(Restrict::and(restricts))
        } else {
            None
        }
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.name(), self.path())
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

    fn categories(&self) -> IndexSet<String> {
        // use profiles/categories from repos, falling back to raw fs dirs
        let mut categories: IndexSet<_> = self
            .trees()
            .flat_map(|r| r.metadata().categories().clone())
            .collect();
        categories.sort();
        if categories.is_empty() {
            self.category_dirs()
        } else {
            categories
        }
    }

    fn packages(&self, cat: &str) -> IndexSet<String> {
        let path = self.path().join(cat.strip_prefix('/').unwrap_or(cat));
        let filter = |e: &DirEntry| -> bool { is_dir(e) && !is_hidden(e) };
        let pkgs = sorted_dir_list(&path).into_iter().filter_entry(filter);
        let mut v = IndexSet::new();
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
                    Ok(_) => {
                        v.insert(s.into());
                    }
                    Err(e) => warn!("{}: {e}: {path:?}", self.id()),
                },
                None => warn!("{}: non-unicode path: {path:?}", self.id()),
            }
        }
        v
    }

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        let path = build_from_paths!(
            self.path(),
            cat.strip_prefix('/').unwrap_or(cat),
            pkg.strip_prefix('/').unwrap_or(pkg)
        );
        let ebuilds = sorted_dir_list(&path).into_iter().filter_entry(is_ebuild);
        let mut versions = IndexSet::new();
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
                        Ok(v) => {
                            versions.insert(v);
                        }
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
    iter: IterCpv<'a>,
    repo: &'a Repo,
}

impl<'a> IterRaw<'a> {
    fn new(repo: &'a Repo, restrict: Option<&Restrict>) -> Self {
        let iter = IterCpv::new(repo, restrict);
        Self { iter, repo }
    }
}

impl<'a> Iterator for IterRaw<'a> {
    type Item = RawPkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        for cpv in &mut self.iter {
            match RawPkg::new(cpv, self.repo) {
                Ok(pkg) => return Some(pkg),
                Err(e) => warn!("{}: {e}", self.repo.id()),
            }
        }
        None
    }
}

pub struct IterCpv<'a> {
    iter: Box<dyn Iterator<Item = Cpv> + 'a>,
}

impl<'a> IterCpv<'a> {
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

        let restricts = (cat.as_deref(), pkg.as_deref(), ver.as_deref());

        Self {
            iter: if let (Some(cat), Some(pkg), Some(ver)) = restricts {
                // specific package restriction
                let cpv = Cpv::try_from((cat, pkg, ver)).expect("dep restrict failed");
                Box::new(iter::once(cpv))
            } else {
                // complex restriction filtering
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
                        .flat_map(|s| repo.category_cpvs(&s))
                        .filter(move |cpv| pkg_restrict.matches(cpv)),
                )
            },
        }
    }
}

impl<'a> Iterator for IterCpv<'a> {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
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

pub struct IterCpvRestrict<'a> {
    iter: IterCpv<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for IterCpvRestrict<'a> {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|cpv| self.restrict.matches(cpv))
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
    use crate::test::{assert_ordered_eq, assert_unordered_eq, TEST_DATA};

    use super::*;

    #[test]
    fn test_masters() {
        let mut config = Config::default();
        let repos_path = TEST_DATA.path.join("repos");

        // none
        let repo = Repo::from_path("a", 0, repos_path.join("dependent-primary")).unwrap();
        let repo = config
            .add_repo_path(repo.id(), 0, repo.path().as_str(), false)
            .unwrap();
        let repo = repo.as_ebuild().unwrap();
        assert!(repo.masters().next().is_none());
        let trees: Vec<_> = repo.trees().map(|r| r.id().to_string()).collect();
        assert_eq!(trees, ["a"]);

        // nonexistent
        let repo = Repo::from_path("test", 0, repos_path.join("dependent-nonexistent")).unwrap();
        let r = config.add_repo_path(repo.id(), 0, repo.path().as_str(), false);
        assert_err_re!(r, "^.* unconfigured repos: nonexistent1, nonexistent2$");

        // single
        let repo = Repo::from_path("b", 0, repos_path.join("dependent-secondary")).unwrap();
        let repo = config
            .add_repo_path(repo.id(), 0, repo.path().as_str(), false)
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
        assert_ordered_eq(repo.categories(), ["cat"]);
        fs::create_dir(repo.path().join("a-cat")).unwrap();
        fs::create_dir(repo.path().join("z-cat")).unwrap();
        assert_ordered_eq(repo.categories(), ["a-cat", "cat", "z-cat"]);
    }

    #[test]
    fn test_packages() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();

        assert!(repo.packages("cat").is_empty());
        fs::create_dir_all(repo.path().join("cat/pkg")).unwrap();
        assert_ordered_eq(repo.packages("cat"), ["pkg"]);
        fs::create_dir_all(repo.path().join("a-cat/pkg-z")).unwrap();
        fs::create_dir_all(repo.path().join("a-cat/pkg-a")).unwrap();
        assert_ordered_eq(repo.packages("a-cat"), ["pkg-a", "pkg-z"]);
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
        assert_ordered_eq(repo.versions("cat", "pkg"), [ver("1")]);

        // unmatching ebuilds are ignored
        fs::File::create(repo.path().join("cat/pkg/foo-2.ebuild")).unwrap();
        assert_ordered_eq(repo.versions("cat", "pkg"), [ver("1")]);

        // wrongly named files are ignored
        fs::File::create(repo.path().join("cat/pkg/pkg-2.txt")).unwrap();
        fs::File::create(repo.path().join("cat/pkg/pkg-2..ebuild")).unwrap();
        fs::File::create(repo.path().join("cat/pkg/pkg-2ebuild")).unwrap();
        assert_ordered_eq(repo.versions("cat", "pkg"), [ver("1")]);

        fs::File::create(repo.path().join("cat/pkg/pkg-2.ebuild")).unwrap();
        assert_ordered_eq(repo.versions("cat", "pkg"), [ver("1"), ver("2")]);

        fs::create_dir_all(repo.path().join("a-cat/pkg10a")).unwrap();
        fs::File::create(repo.path().join("a-cat/pkg10a/pkg10a-0-r0.ebuild")).unwrap();
        assert_ordered_eq(repo.versions("a-cat", "pkg10a"), [ver("0-r0")]);
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

    #[test]
    fn test_pkg_metadata_regen() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();

        let data = indoc::indoc! {r#"
            EAPI="8"
            DESCRIPTION="testing metadata generation"
            SLOT=0
        "#};
        t.create_ebuild_raw("cat/pkg-1", data).unwrap();

        repo.pkg_metadata_regen(1, false, false).unwrap();

        let metadata = indoc::indoc! {r"
            DEFINED_PHASES=-
            DESCRIPTION=testing metadata generation
            EAPI=8
            SLOT=0
            _md5_=ea4f236b663902c60595f1422d1544f3
        "};
        let path = repo.metadata().cache_path().join("cat/pkg-1");
        assert_eq!(fs::read_to_string(path).unwrap(), metadata);
    }
}
