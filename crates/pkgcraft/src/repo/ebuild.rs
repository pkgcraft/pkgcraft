use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::path::Path;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, OnceLock, Weak};
use std::{fmt, fs, io, iter, thread};

use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::{bounded, Receiver, RecvError, Sender};
use indexmap::{IndexMap, IndexSet};
use itertools::{Either, Itertools};
use once_cell::sync::Lazy;
use roxmltree::Document;
use tracing::warn;
use walkdir::{DirEntry, WalkDir};

use crate::config::{RepoConfig, Settings};
use crate::dep::{self, Cpv, Operator, Version};
use crate::eapi::Eapi;
use crate::files::{
    has_ext, is_dir_utf8, is_file, is_hidden, is_hidden_utf8, sorted_dir_list, sorted_dir_list_utf8,
};
use crate::macros::build_from_paths;
use crate::pkg::ebuild::{
    self,
    metadata::{Manifest, XmlMetadata},
};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict, Restriction};
use crate::{Error, COLLAPSE_LAZY_FIELDS};

use super::{make_repo_traits, Contains, PkgRepository, Repo as BaseRepo, RepoFormat, Repository};

mod cache;
pub mod configured;
mod eclass;
pub use eclass::Eclass;
mod metadata;
pub mod temp;
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
            let mut pkg_cache = HashMap::<_, (_, Arc<T>)>::new();
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
    fn get(&self, cpv: &Cpv<String>) -> Arc<T> {
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

#[derive(Default)]
pub struct Repo {
    id: String,
    config: RepoConfig,
    metadata: Metadata,
    masters: OnceLock<Vec<Weak<Self>>>,
    trees: OnceLock<Vec<Weak<Self>>>,
    arches: OnceLock<IndexSet<String>>,
    licenses: OnceLock<IndexSet<String>>,
    license_groups: OnceLock<HashMap<String, HashSet<String>>>,
    mirrors: OnceLock<IndexMap<String, IndexSet<String>>>,
    eclasses: OnceLock<IndexSet<Eclass>>,
    xml_cache: OnceLock<Cache<XmlMetadata>>,
    manifest_cache: OnceLock<Cache<Manifest>>,
    categories_xml: OnceLock<IndexMap<String, String>>,
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

impl PartialEq for Repo {
    fn eq(&self, other: &Self) -> bool {
        self.path() == other.path()
    }
}

impl Eq for Repo {}

impl Hash for Repo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path().hash(state);
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
            metadata: Metadata::try_new(id.as_ref(), path)?,
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

            if COLLAPSE_LAZY_FIELDS.load(Relaxed) {
                // metadata generation requires these fields to be collapsed
                self.eclasses();
                self.arches();
                self.licenses();
            }

            Ok(())
        } else {
            let repos = nonexistent.join(", ");
            Err(Error::InvalidRepo {
                id: self.id().to_string(),
                err: format!("unconfigured repos: {repos}"),
            })
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
    pub fn masters(&self) -> impl DoubleEndedIterator<Item = Arc<Self>> + '_ {
        self.masters
            .get()
            .expect("finalize() uncalled")
            .iter()
            .map(|p| p.upgrade().expect("unconfigured repo"))
    }

    /// Return the complete, repo inheritance set for the repo.
    pub fn trees(&self) -> impl DoubleEndedIterator<Item = Arc<Self>> + '_ {
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

    /// Return the set of inherited eclasses sorted by name.
    pub fn eclasses(&self) -> &IndexSet<Eclass> {
        self.eclasses.get_or_init(|| {
            let mut eclasses: IndexSet<_> = self
                .trees()
                .rev()
                .flat_map(|r| r.metadata().eclasses().clone().into_iter())
                .collect();
            eclasses.sort();
            eclasses
        })
    }

    /// Return a repo's category dirs from the filesystem.
    pub fn category_dirs(&self) -> IndexSet<String> {
        let entries = match sorted_dir_list_utf8(self.path()) {
            Ok(vals) => vals,
            Err(e) => {
                warn!("{}: {}: {e}", self.id(), self.path());
                return Default::default();
            }
        };

        entries
            .into_iter()
            .filter(|e| {
                is_dir_utf8(e) && !is_hidden_utf8(e) && !FAKE_CATEGORIES.contains(e.file_name())
            })
            .filter_map(|entry| {
                let path = entry.path();
                match dep::parse::category(entry.file_name()) {
                    Ok(_) => Some(entry.file_name().to_string()),
                    Err(e) => {
                        warn!("{}: {path}: {e}", self.id());
                        None
                    }
                }
            })
            .collect()
    }

    /// Return the mapping of repo categories to their descriptions.
    pub fn categories_xml(&self) -> &IndexMap<String, String> {
        // parse a category's metadata.xml data
        let parse_xml = |data: &str| -> crate::Result<Option<String>> {
            Document::parse(data)
                .map_err(|e| Error::InvalidValue(format!("failed parsing category xml: {e}")))
                .map(|doc| {
                    doc.root_element().children().find_map(|node| {
                        let lang = node.attribute("lang").unwrap_or("en");
                        if node.tag_name().name() == "longdescription" && lang == "en" {
                            node.text().map(|s| s.split_whitespace().join(" "))
                        } else {
                            None
                        }
                    })
                })
        };

        self.categories_xml.get_or_init(|| {
            self.categories()
                .iter()
                .filter_map(|cat| {
                    let path = build_from_paths!(self.path(), cat, "metadata.xml");
                    let desc = fs::read_to_string(&path)
                        .map_err(|e| Error::IO(format!("failed reading category xml: {e}")))
                        .and_then(|s| parse_xml(&s));
                    match desc {
                        Ok(Some(desc)) => Some((cat.to_string(), desc)),
                        Ok(_) => None,
                        Err(e) => {
                            warn!("{}: {path}: {e}", self.id());
                            None
                        }
                    }
                })
                .collect()
        })
    }

    /// Convert a relative ebuild file repo path into a CPV.
    fn cpv_from_ebuild_path<P: AsRef<Path>>(&self, path: P) -> crate::Result<Cpv<String>> {
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
        Cpv::try_new(format!("{cat}/{p}"))
            .map_err(|_| err("invalid CPV"))
            .and_then(|a| {
                if a.package() == pkg {
                    Ok(a)
                } else {
                    Err(err("mismatched package dir"))
                }
            })
    }

    /// Return the set of inherited architectures sorted by name.
    pub fn arches(&self) -> &IndexSet<String> {
        self.arches.get_or_init(|| {
            self.trees()
                .rev()
                .flat_map(|r| r.metadata().arches().clone().into_iter())
                .collect()
        })
    }

    /// Return the set of inherited licenses sorted by name.
    pub fn licenses(&self) -> &IndexSet<String> {
        self.licenses.get_or_init(|| {
            let mut licenses: IndexSet<_> = self
                .trees()
                .rev()
                .flat_map(|r| r.metadata().licenses().clone().into_iter())
                .collect();
            licenses.sort();
            licenses
        })
    }

    /// Return the mapping of license groups merged via inheritance.
    pub fn license_groups(&self) -> &HashMap<String, HashSet<String>> {
        self.license_groups.get_or_init(|| {
            let mut group_map = self.metadata().license_groups().clone();
            self.masters()
                .flat_map(|r| r.metadata().license_groups().clone().into_iter())
                .for_each(|(name, set)| {
                    group_map.entry(name).or_default().extend(set);
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
    pub(crate) fn pkg_xml(&self, cpv: &Cpv<String>) -> Arc<XmlMetadata> {
        self.xml_cache
            .get_or_init(|| Cache::<XmlMetadata>::new(self.arc()))
            .get(cpv)
    }

    /// Return the shared manifest data for a given package.
    pub(crate) fn pkg_manifest(&self, cpv: &Cpv<String>) -> Arc<Manifest> {
        self.manifest_cache
            .get_or_init(|| Cache::<Manifest>::new(self.arc()))
            .get(cpv)
    }

    /// Return the sorted set of Cpvs in a given category.
    fn category_cpvs(&self, category: &str) -> IndexSet<Cpv<String>> {
        // filter invalid ebuild paths
        let filter_path = |r: walkdir::Result<DirEntry>| -> Option<Cpv<String>> {
            match r {
                Ok(e) => match self.cpv_from_ebuild_path(e.path()) {
                    Ok(cpv) => Some(cpv),
                    Err(e) => {
                        warn!("{}: {e}", self.id());
                        None
                    }
                },
                Err(e) => {
                    if e.io_error()
                        .map(|e| e.kind() != io::ErrorKind::NotFound)
                        .unwrap_or(true)
                    {
                        warn!("{}: failed walking repo: {e}", self.id());
                    }
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

    /// Create a package metadata cache regeneration runner.
    pub fn metadata_regen(&self) -> cache::CacheBuilder {
        cache::CacheBuilder::new(self)
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

    /// Return a configured repo using the given config settings.
    pub fn configure<T: Into<Arc<Settings>>>(&self, settings: T) -> configured::Repo {
        configured::Repo::new(self.arc(), settings.into())
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.name(), self.path())
    }
}

impl PkgRepository for Repo {
    type Pkg<'a> = ebuild::Pkg<'a> where Self: 'a;
    type IterCpv<'a> = IterCpv<'a> where Self: 'a;
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
        let entries = match sorted_dir_list_utf8(&path) {
            Ok(vals) => vals,
            Err(e) => {
                warn!("{}: {path}: {e}", self.id());
                return Default::default();
            }
        };

        entries
            .into_iter()
            .filter(|e| is_dir_utf8(e) && !is_hidden_utf8(e))
            .filter_map(|entry| {
                let path = entry.path();
                match dep::parse::package(entry.file_name()) {
                    Ok(_) => Some(entry.file_name().to_string()),
                    Err(e) => {
                        warn!("{}: {path}: {e}", self.id());
                        None
                    }
                }
            })
            .collect()
    }

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version<String>> {
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
                    match Version::try_new(&pf[pn.len() + 1..]) {
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

    fn iter_cpv(&self) -> IterCpv {
        IterCpv::new(self, None)
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
    type Item = ebuild::Pkg<'a>;
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
    type Item = ebuild::Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        for raw_pkg in &mut self.iter {
            match raw_pkg.try_into() {
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
    type Item = ebuild::raw::Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        for cpv in &mut self.iter {
            match ebuild::raw::Pkg::try_new(cpv, self.repo) {
                Ok(pkg) => return Some(pkg),
                Err(e) => warn!("{}: {e}", self.repo.id()),
            }
        }
        None
    }
}

pub struct IterCpv<'a> {
    iter: Box<dyn Iterator<Item = Cpv<String>> + 'a>,
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
                                ver = Some(v.without_op());
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
    type Item = Cpv<String>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub struct IterRestrict<'a> {
    iter: Iter<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for IterRestrict<'a> {
    type Item = ebuild::Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|pkg| self.restrict.matches(pkg))
    }
}

pub struct IterCpvRestrict<'a> {
    iter: IterCpv<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for IterCpvRestrict<'a> {
    type Item = Cpv<String>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|cpv| self.restrict.matches(cpv))
    }
}

pub struct IterRawRestrict<'a> {
    iter: IterRaw<'a>,
    restrict: Restrict,
}

impl<'a> Iterator for IterRawRestrict<'a> {
    type Item = ebuild::raw::Pkg<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|pkg| self.restrict.matches(pkg))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tracing_test::traced_test;

    use crate::config::Config;
    use crate::dep::Dep;
    use crate::eapi::{EAPI8, EAPI_LATEST_OFFICIAL};
    use crate::macros::*;
    use crate::pkg::Package;
    use crate::repo::ebuild::temp::Repo as TempRepo;
    use crate::repo::Contains;
    use crate::test::{assert_ordered_eq, assert_unordered_eq, TEST_DATA};

    use super::*;

    #[test]
    fn test_masters() {
        let mut config = Config::default();
        let test_path = &TEST_DATA.path;

        // none
        let repo = Repo::from_path("a", 0, test_path.join("repos/primary")).unwrap();
        let repo = config
            .add_repo_path(repo.id(), 0, repo.path().as_str(), false)
            .unwrap();
        let repo = repo.as_ebuild().unwrap();
        assert!(repo.masters().next().is_none());
        let trees: Vec<_> = repo.trees().map(|r| r.id().to_string()).collect();
        assert_eq!(trees, ["a"]);

        // nonexistent
        let repo = Repo::from_path("test", 0, test_path.join("repos/masters-invalid")).unwrap();
        let r = config.add_repo_path(repo.id(), 0, repo.path().as_str(), false);
        assert_err_re!(r, "^.* unconfigured repos: nonexistent1, nonexistent2$");

        // single
        let repo = Repo::from_path("b", 0, test_path.join("repos/secondary")).unwrap();
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
        // repos lacking profiles/eapi file use the latest EAPI
        let t = TempRepo::new("test", None, 0, None).unwrap();
        assert_eq!(t.repo().eapi(), *EAPI_LATEST_OFFICIAL);

        // explicit repo EAPI
        let t = TempRepo::new("test", None, 0, Some(&*EAPI8)).unwrap();
        assert_eq!(t.repo().eapi(), &*EAPI8);
    }

    #[test]
    fn test_len() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();

        assert_eq!(repo.len(), 0);
        assert!(repo.is_empty());
        t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        assert_eq!(repo.len(), 1);
        assert!(!repo.is_empty());
        t.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
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
        let ver = |s: &str| Version::try_new(s).unwrap();

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
        t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        assert!(repo.contains("cat/pkg"));
        assert!(repo.contains("cat/pkg/pkg-1.ebuild"));
        assert!(!repo.contains("pkg-1.ebuild"));

        // cpv
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        assert!(repo.contains(&cpv));
        let cpv = Cpv::try_new("cat/pkg-2").unwrap();
        assert!(!repo.contains(&cpv));

        // unversioned dep
        let d = Dep::try_new("cat/pkg").unwrap();
        assert!(repo.contains(&d));
        let d = Dep::try_new("cat/pkg-a").unwrap();
        assert!(!repo.contains(&d));
    }

    #[test]
    fn test_iter() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();
        t.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        t.create_raw_pkg("cat1/pkg-1", &[]).unwrap();
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
        t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        t.create_raw_pkg("cat/pkg-2", &[]).unwrap();

        // single match via CPV
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
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
            ("EAPI=a", "unsupported EAPI: a"),
            ("SLOT=", "missing required value: SLOT"),
        ] {
            let mut config = Config::default();
            let t = config.temp_repo("test", 0, None).unwrap();
            t.create_raw_pkg("cat/pkg-0", &[data]).ok();
            let mut iter = t.repo().iter();
            assert!(iter.next().is_none());
            assert_logs_re!(format!("test: invalid pkg: .+: {err}$"));
        }
    }

    #[test]
    fn test_eclasses() {
        let repo1 = TEST_DATA.ebuild_repo("primary").unwrap();
        assert_unordered_eq(repo1.eclasses().iter().map(|e| e.name()), ["a", "c"]);
        let repo2 = TEST_DATA.ebuild_repo("secondary").unwrap();
        assert_unordered_eq(repo2.eclasses().iter().map(|e| e.name()), ["a", "b", "c"]);
        // verify the overridden eclass is from the secondary repo
        let overridden_eclass = repo2.eclasses().get("c").unwrap();
        assert!(overridden_eclass.path().starts_with(repo2.path()));
    }

    #[test]
    fn test_arches() {
        let repo = TEST_DATA.ebuild_repo("primary").unwrap();
        assert_unordered_eq(repo.arches(), ["x86"]);
        let repo = TEST_DATA.ebuild_repo("secondary").unwrap();
        assert_unordered_eq(repo.arches(), ["amd64", "x86"]);
    }

    #[test]
    fn test_licenses() {
        let repo = TEST_DATA.ebuild_repo("primary").unwrap();
        assert_unordered_eq(repo.licenses(), ["a"]);
        let repo = TEST_DATA.ebuild_repo("secondary").unwrap();
        assert_unordered_eq(repo.licenses(), ["a", "b"]);
    }

    #[test]
    fn test_categories_xml() {
        let repo = TEST_DATA.ebuild_repo("xml").unwrap();
        assert_eq!(repo.categories_xml().get("good").unwrap(), "good");
        // categories with invalid XML data don't have entries
        assert!(repo.categories_xml().get("bad").is_none());
        // categories without XML data don't have entries
        assert!(repo.categories_xml().get("pkg").is_none());
        // nonexistent categories don't have entries
        assert!(repo.categories_xml().get("nonexistent").is_none());
    }

    #[traced_test]
    #[test]
    fn test_metadata_regen_errors() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        let repo = t.repo();

        // create a large number of packages with a subshelled, invalid scope builtin call
        for pv in 0..50 {
            let data = indoc::indoc! {r#"
                EAPI=8
                DESCRIPTION="testing metadata generation error handling"
                SLOT=0
                VAR=$(best_version cat/pkg)
            "#};
            t.create_raw_pkg_from_str(format!("cat/pkg-{pv}"), data)
                .unwrap();
        }

        // run regen asserting that errors occurred
        let r = repo.metadata_regen().suppress(true).run();
        assert!(r.is_err());

        // verify all pkgs caused logged errors
        for pv in 0..50 {
            assert_logs_re!(format!(
                "invalid pkg: cat/pkg-{pv}::test: line 4: best_version: error: disabled in global scope$"
            ));
        }
    }
}
