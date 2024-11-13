use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock, OnceLock};
use std::{fmt, fs, io, iter, thread};

use camino::{Utf8Path, Utf8PathBuf};
use crossbeam_channel::{bounded, RecvError, Sender};
use indexmap::{IndexMap, IndexSet};
use itertools::{Either, Itertools};
use rayon::prelude::*;
use tracing::warn;

use crate::config::{RepoConfig, Settings};
use crate::dep::{self, Cpn, Cpv, Dep, Operator, Version};
use crate::eapi::Eapi;
use crate::files::{has_ext_utf8, is_dir_utf8, is_file_utf8, is_hidden_utf8, sorted_dir_list_utf8};
use crate::macros::build_path;
use crate::pkg::ebuild::keyword::Arch;
use crate::pkg::ebuild::{self, manifest::Manifest, xml};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict, Restriction};
use crate::traits::Intersects;
use crate::xml::parse_xml_with_dtd;
use crate::Error;

use super::{make_repo_traits, Contains, PkgRepository, Repo, RepoFormat, Repository};

pub mod cache;
pub mod configured;
mod eclass;
pub use eclass::Eclass;
mod metadata;
pub mod temp;
pub use metadata::Metadata;

// root level directories that aren't categories
static FAKE_CATEGORIES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    ["eclass", "profiles", "metadata", "licenses"]
        .into_iter()
        .collect()
});

/// Shared data cache trait.
pub(crate) trait ArcCacheData: Default {
    const RELPATH: &'static str;
    fn parse(data: &str) -> crate::Result<Self>;
}

#[derive(Debug)]
struct ArcCache<T>
where
    T: ArcCacheData + Send + Sync,
{
    thread: Option<thread::JoinHandle<()>>,
    tx: Sender<Msg<T>>,
}

enum Msg<T: ArcCacheData + Send + Sync> {
    Key(String, Sender<Arc<T>>),
    Stop,
}

impl<T> ArcCache<T>
where
    T: ArcCacheData + Send + Sync + 'static,
{
    fn new(repo: EbuildRepo) -> Self {
        let (tx, rx) = bounded(10);

        let thread = thread::spawn(move || {
            // TODO: limit cache size using an LRU cache with set capacity
            let mut cache = HashMap::<_, (_, Arc<T>)>::new();
            loop {
                match rx.recv() {
                    Ok(Msg::Stop) | Err(RecvError) => break,
                    Ok(Msg::Key(s, tx)) => {
                        let path = build_path!(repo.path(), &s, T::RELPATH);
                        let data = fs::read_to_string(&path)
                            .map_err(|e| {
                                if e.kind() != io::ErrorKind::NotFound {
                                    warn!("{}: failed reading: {path}: {e}", repo.id());
                                }
                            })
                            .unwrap_or_default();

                        // evict cache entries based on file content hash
                        let hash = blake3::hash(data.as_bytes());

                        let val = match cache.get(&s) {
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
                                cache.insert(s, (hash, val.clone()));
                                val
                            }
                        };

                        tx.send(val).expect("failed sending shared pkg data");
                    }
                }
            }
        });

        Self { thread: Some(thread), tx }
    }

    /// Get the cache data related to a given package Cpv.
    fn get(&self, cpn: &Cpn) -> crate::Result<Arc<T>> {
        let (tx, rx) = bounded(0);
        self.tx.send(Msg::Key(cpn.to_string(), tx)).map_err(|e| {
            Error::InvalidValue(format!("failed requesting pkg manifest data: {cpn}: {e}"))
        })?;
        rx.recv().map_err(|e| {
            Error::InvalidValue(format!("failed receiving pkg manifest data: {cpn}: {e}"))
        })
    }
}

// Note that the thread will currently be killed without joining on exit since
// ArcCache is contained in a OnceLock that doesn't call drop().
impl<T> Drop for ArcCache<T>
where
    T: ArcCacheData + Send + Sync,
{
    fn drop(&mut self) {
        self.tx.send(Msg::Stop).unwrap();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

#[derive(Debug)]
struct InternalEbuildRepo {
    metadata: Metadata,
    config: RepoConfig,
    masters: OnceLock<Vec<EbuildRepo>>,
    arches: OnceLock<IndexSet<Arch>>,
    licenses: OnceLock<IndexSet<String>>,
    license_groups: OnceLock<IndexMap<String, IndexSet<String>>>,
    mirrors: OnceLock<IndexMap<String, IndexSet<String>>>,
    eclasses: OnceLock<IndexSet<Eclass>>,
    use_expand: OnceLock<IndexMap<String, IndexMap<String, String>>>,
    metadata_cache: OnceLock<ArcCache<xml::Metadata>>,
    manifest_cache: OnceLock<ArcCache<Manifest>>,
    categories_xml: OnceLock<IndexMap<String, String>>,
}

#[derive(Clone)]
pub struct EbuildRepo(Arc<InternalEbuildRepo>);

impl fmt::Debug for EbuildRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Repo")
            .field("id", &self.id())
            .field("repo_config", self.repo_config())
            .field("name", &self.name())
            .finish()
    }
}

impl PartialEq for EbuildRepo {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id() && self.repo_config() == other.repo_config()
    }
}

impl Eq for EbuildRepo {}

impl Hash for EbuildRepo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path().hash(state);
    }
}

impl From<&EbuildRepo> for Restrict {
    fn from(repo: &EbuildRepo) -> Self {
        repo.restrict_from_path(repo.path()).unwrap()
    }
}

make_repo_traits!(EbuildRepo);

impl EbuildRepo {
    /// Create an ebuild repo from a given path.
    pub(crate) fn from_path<S, P>(id: S, priority: i32, path: P) -> crate::Result<Self>
    where
        S: AsRef<str>,
        P: AsRef<Utf8Path>,
    {
        let path = path.as_ref();
        let metadata = Metadata::try_new(id.as_ref(), path)?;
        let config = RepoConfig {
            location: Utf8PathBuf::from(path),
            priority,
            ..Default::default()
        };

        Ok(Self(Arc::new(InternalEbuildRepo {
            metadata,
            config,
            masters: OnceLock::new(),
            arches: OnceLock::new(),
            licenses: OnceLock::new(),
            license_groups: OnceLock::new(),
            mirrors: OnceLock::new(),
            eclasses: OnceLock::new(),
            use_expand: OnceLock::new(),
            metadata_cache: OnceLock::new(),
            manifest_cache: OnceLock::new(),
            categories_xml: OnceLock::new(),
        })))
    }

    /// Finalize the repo, collapsing repo dependencies into references.
    pub(super) fn finalize(&self, existing_repos: &IndexMap<String, Repo>) -> crate::Result<()> {
        // skip finalized, stand-alone repos
        if self.0.masters.get().is_some() {
            return Ok(());
        }

        let (masters, nonexistent): (Vec<_>, Vec<_>) =
            self.metadata().config.masters.iter().partition_map(|id| {
                match existing_repos.get(id).and_then(|r| r.as_ebuild()) {
                    Some(r) => Either::Left(r.clone()),
                    None => Either::Right(id.as_str()),
                }
            });

        if !nonexistent.is_empty() {
            let repos = nonexistent.join(", ");
            return Err(Error::InvalidValue(format!("nonexistent masters: {repos}")));
        }

        self.0
            .masters
            .set(masters)
            .map_err(|_| Error::InvalidValue("already initialized".to_string()))
    }

    /// Collapse required lazy fields for metadata regeneration that leverages process-based
    /// parallelism. If this is not called beforehand, each spawned process will reinitialize
    /// all lazy fields they use often slowing down runtime considerably.
    fn collapse_cache_regen(&self) {
        self.eclasses();
        self.arches();
        self.licenses();
    }

    /// Return the repo config.
    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.0.config
    }

    pub fn metadata(&self) -> &Metadata {
        &self.0.metadata
    }

    /// Return the repo EAPI (set in profiles/eapi).
    pub fn eapi(&self) -> &'static Eapi {
        self.metadata().eapi
    }

    /// Return the repo inheritance sequence.
    pub fn masters(&self) -> &[Self] {
        self.0.masters.get().expect("finalize() uncalled")
    }

    /// Return the complete repo inheritance sequence.
    pub fn trees(&self) -> impl DoubleEndedIterator<Item = &Self> {
        self.masters().iter().chain([self])
    }

    /// Return the ordered map of inherited eclasses.
    pub fn eclasses(&self) -> &IndexSet<Eclass> {
        self.0.eclasses.get_or_init(|| {
            let mut eclasses: IndexSet<_> = self
                .trees()
                .rev()
                .flat_map(|r| r.metadata().eclasses().clone())
                .collect();
            eclasses.sort();
            eclasses
        })
    }

    /// Return the ordered map of inherited USE_EXPAND flags.
    pub fn use_expand(&self) -> &IndexMap<String, IndexMap<String, String>> {
        self.0.use_expand.get_or_init(|| {
            let mut use_expand: IndexMap<_, _> = self
                .trees()
                .rev()
                .flat_map(|r| r.metadata().use_expand().clone())
                .collect();
            use_expand.sort_keys();
            use_expand
        })
    }

    /// Return a repo's category dirs from the filesystem.
    fn category_dirs(&self) -> IndexSet<String> {
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
            parse_xml_with_dtd(data)
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

        self.0.categories_xml.get_or_init(|| {
            self.categories()
                .iter()
                .filter_map(|cat| {
                    let path = build_path!(self.path(), cat, "metadata.xml");
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

    /// Convert an ebuild file path into a Cpv.
    fn cpv_from_path(&self, path: &Utf8Path) -> crate::Result<Cpv> {
        let err =
            |s: &str| -> Error { Error::InvalidValue(format!("invalid ebuild path: {path}: {s}")) };
        let relpath = path.strip_prefix(self.path()).unwrap_or(path);
        let (cat, pkg, file) = relpath
            .components()
            .map(|s| s.as_str())
            .collect_tuple()
            .ok_or_else(|| err("mismatched path components"))?;
        let p = file
            .strip_suffix(".ebuild")
            .ok_or_else(|| err("missing ebuild ext"))?;
        Cpv::try_new(format!("{cat}/{p}"))
            .map_err(|_| err("invalid Cpv"))
            .and_then(|a| {
                if a.package() == pkg {
                    Ok(a)
                } else {
                    Err(err("mismatched package dir"))
                }
            })
    }

    /// Return the set of inherited architectures sorted by name.
    pub fn arches(&self) -> &IndexSet<Arch> {
        self.0.arches.get_or_init(|| {
            let mut arches: IndexSet<_> = self
                .trees()
                .rev()
                .flat_map(|r| r.metadata().arches().clone())
                .collect();
            arches.sort();
            arches
        })
    }

    /// Return the set of inherited licenses sorted by name.
    pub fn licenses(&self) -> &IndexSet<String> {
        self.0.licenses.get_or_init(|| {
            let mut licenses: IndexSet<_> = self
                .trees()
                .rev()
                .flat_map(|r| r.metadata().licenses().clone())
                .collect();
            licenses.sort();
            licenses
        })
    }

    /// Return the mapping of license groups merged via inheritance.
    pub fn license_groups(&self) -> &IndexMap<String, IndexSet<String>> {
        self.0.license_groups.get_or_init(|| {
            let mut license_groups: IndexMap<_, _> = self
                .trees()
                .rev()
                .flat_map(|r| r.metadata().license_groups().clone())
                .collect();
            license_groups.sort_keys();
            license_groups
        })
    }

    /// Return the set of mirrors merged via inheritance.
    pub fn mirrors(&self) -> &IndexMap<String, IndexSet<String>> {
        self.0.mirrors.get_or_init(|| {
            let mut mirrors: IndexMap<_, _> = self
                .trees()
                .rev()
                .flat_map(|r| r.metadata().mirrors().clone())
                .collect();
            mirrors.sort_keys();
            mirrors
        })
    }

    /// Return the shared metadata for a given package.
    pub fn pkg_metadata(&self, cpn: &Cpn) -> crate::Result<Arc<xml::Metadata>> {
        self.0
            .metadata_cache
            .get_or_init(|| ArcCache::<xml::Metadata>::new(self.clone()))
            .get(cpn)
    }

    /// Return the shared manifest for a given package.
    pub fn pkg_manifest(&self, cpn: &Cpn) -> crate::Result<Arc<Manifest>> {
        self.0
            .manifest_cache
            .get_or_init(|| ArcCache::<Manifest>::new(self.clone()))
            .get(cpn)
    }

    /// Return the sorted set of Cpvs from a given category.
    pub fn cpvs_from_category(&self, category: &str) -> IndexSet<Cpv> {
        let path = build_path!(self.path(), category);
        if let Ok(entries) = path.read_dir_utf8() {
            let mut cpvs: IndexSet<_> = entries
                .filter_map(|e| e.ok())
                .flat_map(|e| self.cpvs_from_package(category, e.file_name()))
                .collect();
            cpvs.sort();
            cpvs
        } else {
            Default::default()
        }
    }

    /// Return the sorted set of Cpvs from a given package.
    fn cpvs_from_package(&self, category: &str, package: &str) -> IndexSet<Cpv> {
        let path = build_path!(self.path(), category, package);
        if let Ok(entries) = path.read_dir_utf8() {
            let mut cpvs: IndexSet<_> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| self.cpv_from_path(e.path()).ok())
                .collect();
            cpvs.sort();
            cpvs
        } else {
            Default::default()
        }
    }

    pub fn iter_cpn(&self) -> IterCpn {
        IterCpn::new(self.clone(), None)
    }

    /// Return a filtered iterator of unversioned Deps for the repo.
    pub fn iter_cpn_restrict<R: Into<Restrict>>(&self, val: R) -> IterCpnRestrict {
        let restrict = val.into();
        IterCpnRestrict {
            iter: IterCpn::new(self.clone(), Some(&restrict)),
            restrict,
        }
    }

    /// Return an iterator of raw packages for the repo.
    pub fn iter_raw(&self) -> IterRaw {
        IterRaw::new(self, None)
    }

    /// Return a filtered iterator of raw packages for the repo.
    pub fn iter_raw_restrict<R: Into<Restrict>>(&self, val: R) -> IterRawRestrict {
        let restrict = val.into();
        IterRawRestrict {
            iter: IterRaw::new(self, Some(&restrict)),
            restrict,
        }
    }

    /// Retrieve a package from the repo given its [`Cpv`].
    pub fn get_pkg<T: TryInto<Cpv>>(&self, value: T) -> crate::Result<ebuild::Pkg>
    where
        Error: From<<T as TryInto<Cpv>>::Error>,
    {
        let raw_pkg = self.get_pkg_raw(value)?;
        raw_pkg.try_into()
    }

    /// Retrieve a raw package from the repo given its [`Cpv`].
    pub fn get_pkg_raw<T: TryInto<Cpv>>(&self, value: T) -> crate::Result<ebuild::raw::Pkg>
    where
        Error: From<<T as TryInto<Cpv>>::Error>,
    {
        let cpv = value.try_into()?;
        ebuild::raw::Pkg::try_new(cpv, self.clone())
    }

    /// Scan the deprecated package list returning the first match for a given dependency.
    pub fn deprecated(&self, dep: &Dep) -> Option<&Dep> {
        if dep.blocker().is_none() {
            if let Some(pkg) = self
                .metadata()
                .pkg_deprecated()
                .iter()
                .find(|x| x.intersects(dep))
            {
                match (pkg.slot_dep(), dep.slot_dep()) {
                    // deprecated pkg matches all slots
                    (None, _) => return Some(pkg),
                    // deprecated slot dep matches the dependency
                    (Some(s1), Some(s2)) if s1.slot() == s2.slot() => return Some(pkg),
                    // TODO: query slot cache for remaining mismatched variants?
                    _ => (),
                }
            }
        }
        None
    }

    /// Return a configured repo using the given config settings.
    pub fn configure<T: Into<Arc<Settings>>>(&self, settings: T) -> configured::ConfiguredRepo {
        configured::ConfiguredRepo::new(self.clone(), settings.into())
    }
}

impl fmt::Display for EbuildRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.name(), self.path())
    }
}

impl PkgRepository for EbuildRepo {
    type Pkg = ebuild::Pkg;
    type IterCpv = IterCpv;
    type IterCpvRestrict = IterCpvRestrict;
    type Iter = Iter;
    type IterRestrict = IterRestrict;

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
        let path = self.path().join(cat);
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

    fn versions(&self, cat: &str, pkg: &str) -> IndexSet<Version> {
        let path = build_path!(self.path(), cat, pkg);
        let entries = match sorted_dir_list_utf8(&path) {
            Ok(vals) => vals,
            Err(e) => {
                warn!("{}: {path}: {e}", self.id());
                return Default::default();
            }
        };

        let mut versions: IndexSet<_> = entries
            .into_iter()
            .filter(|e| is_file_utf8(e) && !is_hidden_utf8(e) && has_ext_utf8(e, "ebuild"))
            .filter_map(|entry| {
                let path = entry.path();
                let pn = path.parent().unwrap().file_name().unwrap();
                let pf = path.file_stem().unwrap();
                if pn == &pf[..pn.len()] {
                    match Version::try_new(&pf[pn.len() + 1..]) {
                        Ok(v) => return Some(v),
                        Err(e) => warn!("{}: {e}: {path}", self.id()),
                    }
                } else {
                    warn!("{}: unmatched ebuild: {path}", self.id());
                }
                None
            })
            .collect();
        versions.sort();
        versions
    }

    fn iter_cpv(&self) -> IterCpv {
        IterCpv::new(self.clone(), None)
    }

    fn iter_cpv_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpvRestrict {
        let restrict = value.into();
        IterCpvRestrict {
            iter: IterCpv::new(self.clone(), Some(&restrict)),
            restrict,
        }
    }

    fn iter(&self) -> Self::Iter {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, val: R) -> Self::IterRestrict {
        let restrict = val.into();
        IterRestrict {
            iter: Iter::new(self, Some(&restrict)),
            restrict,
        }
    }
}

impl Repository for EbuildRepo {
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

    fn restrict_from_path<P: AsRef<Utf8Path>>(&self, path: P) -> Option<Restrict> {
        // normalize path to inspect relative components
        let path = path.as_ref();
        let mut abspath = if !path.is_absolute() {
            self.path().join(path)
        } else {
            path.to_path_buf()
        };
        abspath = abspath.canonicalize_utf8().ok()?;
        let Ok(relpath) = abspath.strip_prefix(self.path()) else {
            // non-repo path
            return None;
        };

        let mut restricts = vec![];
        let mut cat = "";
        let mut pn = "";
        for s in relpath.components().map(|p| p.as_str()) {
            match &restricts[..] {
                [] if self.categories().contains(s) => {
                    cat = s;
                    restricts.push(DepRestrict::category(s));
                }
                [_] if self.packages(cat).contains(s) => {
                    pn = s;
                    restricts.push(DepRestrict::package(s));
                }
                [_, _] => {
                    if let Some(p) = s.strip_suffix(".ebuild") {
                        if let Ok(cpv) = Cpv::try_new(format!("{cat}/{p}")) {
                            if pn == cpv.package() {
                                restricts.push(DepRestrict::Version(Some(cpv.version)));
                                continue;
                            } else {
                                warn!("{}: unmatched ebuild: {path}", self.id());
                            }
                        }
                    }

                    // don't generate restrictions for non-ebuild path
                    restricts.clear();
                    break;
                }
                _ => {
                    restricts.clear();
                    break;
                }
            }
        }

        if !restricts.is_empty() {
            // package path
            Some(Restrict::and(restricts))
        } else if relpath == "" {
            // repo root path
            Some(Restrict::True)
        } else {
            // non-package path
            Some(Restrict::False)
        }
    }

    fn sync(&self) -> crate::Result<()> {
        self.repo_config().sync()
    }
}

impl Contains<&Cpn> for EbuildRepo {
    fn contains(&self, cpn: &Cpn) -> bool {
        self.path().join(cpn.to_string()).exists()
    }
}

impl Contains<&Cpv> for EbuildRepo {
    fn contains(&self, cpv: &Cpv) -> bool {
        self.path().join(cpv.relpath()).exists()
    }
}

impl Contains<&Dep> for EbuildRepo {
    fn contains(&self, dep: &Dep) -> bool {
        self.iter_restrict(dep).next().is_some()
    }
}

impl IntoIterator for &EbuildRepo {
    type Item = ebuild::Pkg;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        Iter::new(self, None)
    }
}

/// Iterable of valid ebuild packages.
pub struct Iter(IterRaw);

impl Iter {
    fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        Self(IterRaw::new(repo, restrict))
    }
}

impl Iterator for Iter {
    type Item = ebuild::Pkg;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.find_map(|raw_pkg| raw_pkg.try_into().ok())
    }
}

/// Iterable of valid, raw ebuild packages.
pub struct IterRaw {
    iter: IterCpv,
    repo: EbuildRepo,
}

impl IterRaw {
    fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        Self {
            iter: IterCpv::new(repo.clone(), restrict),
            repo: repo.clone(),
        }
    }
}

impl Iterator for IterRaw {
    type Item = ebuild::raw::Pkg;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .find_map(|cpv| ebuild::raw::Pkg::try_new(cpv, self.repo.clone()).ok())
    }
}

/// Iterable of [`Cpn`] objects.
pub struct IterCpn(Box<dyn Iterator<Item = Cpn>>);

impl IterCpn {
    fn new(repo: EbuildRepo, restrict: Option<&Restrict>) -> Self {
        use DepRestrict::{Category, Package};
        use StrRestrict::Equal;
        let mut cat_restricts = vec![];
        let mut pkg_restricts = vec![];

        // extract matching restrictions for optimized iteration
        if let Some(restrict) = restrict {
            let mut match_restrict = |restrict: &Restrict| match restrict {
                Restrict::Dep(Category(r)) => cat_restricts.push(r.clone()),
                Restrict::Dep(Package(r)) => pkg_restricts.push(r.clone()),
                _ => (),
            };

            if let Restrict::And(vals) = restrict {
                vals.iter().for_each(|x| match_restrict(x));
            } else {
                match_restrict(restrict);
            }
        }

        Self(match (&mut *cat_restricts, &mut *pkg_restricts) {
            ([], []) => {
                // TODO: revert to serialized iteration once repos provide parallel iterators
                let mut cpns = repo
                    .categories()
                    .into_par_iter()
                    .flat_map(|cat| {
                        repo.packages(&cat)
                            .into_iter()
                            .map(|pn| Cpn {
                                category: cat.to_string(),
                                package: pn,
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                cpns.par_sort();
                Box::new(cpns.into_iter())
            }
            ([Equal(cat)], [Equal(pn)]) => {
                let cat = std::mem::take(cat);
                let pn = std::mem::take(pn);
                let cpn = Cpn { category: cat, package: pn };
                if repo.contains(&cpn) {
                    Box::new(iter::once(cpn))
                } else {
                    Box::new(iter::empty())
                }
            }
            ([Equal(cat)], _) => {
                let cat = std::mem::take(cat);
                let pkg_restrict = Restrict::and(pkg_restricts);
                Box::new(
                    repo.packages(&cat)
                        .into_iter()
                        .filter(move |pn| pkg_restrict.matches(pn.as_str()))
                        .map(move |pn| Cpn {
                            category: cat.clone(),
                            package: pn,
                        }),
                )
            }
            (_, [Equal(pn)]) => {
                let pn = std::mem::take(pn);
                let cat_restrict = Restrict::and(cat_restricts);
                Box::new(
                    repo.categories()
                        .into_iter()
                        .filter(move |cat| cat_restrict.matches(cat.as_str()))
                        .flat_map(move |cat| {
                            let cpn = Cpn {
                                category: cat,
                                package: pn.to_string(),
                            };
                            if repo.contains(&cpn) {
                                vec![cpn]
                            } else {
                                vec![]
                            }
                        }),
                )
            }
            _ => {
                let cat_restrict = Restrict::and(cat_restricts);
                let pkg_restrict = Restrict::and(pkg_restricts);
                Box::new(
                    repo.categories()
                        .into_iter()
                        .filter(move |cat| cat_restrict.matches(cat.as_str()))
                        .flat_map(move |cat| {
                            repo.packages(&cat)
                                .into_iter()
                                .filter(|pn| pkg_restrict.matches(pn.as_str()))
                                .map(|pn| Cpn {
                                    category: cat.clone(),
                                    package: pn,
                                })
                                .collect::<Vec<_>>()
                        }),
                )
            }
        })
    }
}

impl Iterator for IterCpn {
    type Item = Cpn;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Iterable of [`Cpv`] objects.
pub struct IterCpv(Box<dyn Iterator<Item = Cpv>>);

impl IterCpv {
    fn new(repo: EbuildRepo, restrict: Option<&Restrict>) -> Self {
        use DepRestrict::{Category, Package, Version};
        use StrRestrict::Equal;
        let mut cat_restricts = vec![];
        let mut pkg_restricts = vec![];
        let mut ver_restricts = vec![];

        // extract matching restrictions for optimized iteration
        if let Some(restrict) = restrict {
            let mut match_restrict = |restrict: &Restrict| match restrict {
                Restrict::Dep(r @ Category(_)) => cat_restricts.push(r.clone()),
                Restrict::Dep(r @ Package(_)) => pkg_restricts.push(r.clone()),
                Restrict::Dep(r @ Version(_)) => ver_restricts.push(r.clone()),
                _ => (),
            };

            if let Restrict::And(vals) = restrict {
                vals.iter().for_each(|x| match_restrict(x));
            } else {
                match_restrict(restrict);
            }
        }

        Self(match (&mut *cat_restricts, &mut *pkg_restricts, &mut *ver_restricts) {
            ([], [], []) => {
                // TODO: revert to serialized iteration once repos provide parallel iterators
                let mut cpvs = repo
                    .categories()
                    .into_par_iter()
                    .flat_map(|s| repo.cpvs_from_category(&s))
                    .collect::<Vec<_>>();
                cpvs.par_sort();
                Box::new(cpvs.into_iter())
            }
            ([Category(Equal(cat))], [Package(Equal(pn))], [Version(Some(ver))])
                if ver.op().is_none() || ver.op() == Some(Operator::Equal) =>
            {
                let cpv = Cpv::try_from((cat, pn, ver.without_op())).expect("invalid Cpv");
                if repo.contains(&cpv) {
                    Box::new(iter::once(cpv))
                } else {
                    Box::new(iter::empty())
                }
            }
            ([Category(Equal(cat))], [Package(Equal(pn))], _) => {
                let ver_restrict = Restrict::and(ver_restricts);
                Box::new(
                    repo.cpvs_from_package(cat, pn)
                        .into_iter()
                        .filter(move |cpv| ver_restrict.matches(cpv)),
                )
            }
            ([], [Package(Equal(pn))], _) => {
                let pn = std::mem::take(pn);
                let ver_restrict = Restrict::and(ver_restricts);
                Box::new(repo.categories().into_iter().flat_map(move |cat| {
                    repo.cpvs_from_package(&cat, &pn)
                        .into_iter()
                        .filter(|cpv| ver_restrict.matches(cpv))
                        .collect::<Vec<_>>()
                }))
            }
            _ => {
                let cat_restrict = Restrict::and(cat_restricts);
                let pkg_restrict = Restrict::and(pkg_restricts);
                let ver_restrict = Restrict::and(ver_restricts);
                Box::new(
                    repo.categories()
                        .into_iter()
                        .filter(move |s| cat_restrict.matches(s.as_str()))
                        .flat_map(move |s| repo.cpvs_from_category(&s))
                        .filter(move |cpv| pkg_restrict.matches(cpv))
                        .filter(move |cpv| ver_restrict.matches(cpv)),
                )
            }
        })
    }
}

impl Iterator for IterCpv {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Iterable of valid ebuild packages matching a given restriction.
pub struct IterRestrict {
    iter: Iter,
    restrict: Restrict,
}

impl Iterator for IterRestrict {
    type Item = ebuild::Pkg;

    fn next(&mut self) -> Option<Self::Item> {
        if self.restrict == Restrict::False {
            None
        } else {
            self.iter.find(|pkg| self.restrict.matches(pkg))
        }
    }
}

/// Iterable of [`Cpn`] objects matching a given restriction.
pub struct IterCpnRestrict {
    iter: IterCpn,
    restrict: Restrict,
}

impl Iterator for IterCpnRestrict {
    type Item = Cpn;

    fn next(&mut self) -> Option<Self::Item> {
        if self.restrict == Restrict::False {
            None
        } else {
            self.iter.find(|cpn| self.restrict.matches(cpn))
        }
    }
}

/// Iterable of [`Cpv`] objects matching a given restriction.
pub struct IterCpvRestrict {
    iter: IterCpv,
    restrict: Restrict,
}

impl Iterator for IterCpvRestrict {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        if self.restrict == Restrict::False {
            None
        } else {
            self.iter.find(|cpv| self.restrict.matches(cpv))
        }
    }
}

/// Iterable of valid, raw ebuild packages matching a given restriction.
pub struct IterRawRestrict {
    iter: IterRaw,
    restrict: Restrict,
}

impl Iterator for IterRawRestrict {
    type Item = ebuild::raw::Pkg;

    fn next(&mut self) -> Option<Self::Item> {
        if self.restrict == Restrict::False {
            None
        } else {
            self.iter.find(|pkg| self.restrict.matches(pkg))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::Config;
    use crate::dep::Dep;
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::pkg::Package;
    use crate::repo::Contains;
    use crate::test::{assert_err_re, assert_ordered_eq, TEST_DATA};

    use super::*;

    #[test]
    fn masters() {
        let mut config = Config::default();
        let repos = TEST_DATA.path().join("repos");

        // none
        let repo = config
            .add_repo_path("a", repos.join("valid/primary"), 0, false)
            .unwrap();
        let primary_repo = repo.as_ebuild().unwrap();
        assert!(primary_repo.masters().is_empty());
        assert_ordered_eq!(primary_repo.trees(), [primary_repo]);

        // nonexistent
        let r = config.add_repo_path("test", repos.join("invalid/nonexistent-masters"), 0, false);
        assert_err_re!(r, "^.* nonexistent masters: nonexistent1, nonexistent2$");

        // single
        let repo = config
            .add_repo_path("b", repos.join("valid/secondary"), 0, false)
            .unwrap();
        let secondary_repo = repo.as_ebuild().unwrap();
        assert_ordered_eq!(secondary_repo.masters(), [primary_repo]);
        assert_ordered_eq!(secondary_repo.trees(), [primary_repo, secondary_repo]);
    }

    #[test]
    fn invalid() {
        let repos = TEST_DATA.path().join("repos");

        // invalid profiles/eapi file
        let path = repos.join("invalid/invalid-eapi");
        let r = EbuildRepo::from_path(&path, 0, &path);
        assert_err_re!(
            r,
            format!(r##"^invalid repo: {path}: profiles/eapi: invalid EAPI: "# invalid\\n8""##)
        );

        // nonexistent profiles/repo_name file
        let path = repos.join("invalid/missing-name");
        let r = EbuildRepo::from_path(&path, 0, &path);
        assert_err_re!(
            r,
            format!("^invalid repo: {path}: profiles/repo_name: No such file or directory")
        );
    }

    #[test]
    fn id_and_name() {
        // repo id matches name
        let repo = TEST_DATA.ebuild_repo("primary").unwrap();
        assert_eq!(repo.id(), "primary");
        assert_eq!(repo.name(), "primary");

        // repo id differs from name
        let repo = EbuildRepo::from_path("name", 0, repo.path()).unwrap();
        assert_eq!(repo.id(), "name");
        assert_eq!(repo.name(), "primary");
    }

    #[test]
    fn eapi() {
        let mut config = Config::default();
        let repos = TEST_DATA.path().join("repos");

        // nonexistent profiles/eapi file uses EAPI 0 which isn't supported
        let r = config.add_repo_path("test", repos.join("invalid/unsupported-eapi"), 0, false);
        assert_err_re!(r, "^invalid repo: test: profiles/eapi: unsupported EAPI: 0$");

        // unknown EAPI
        let r = config.add_repo_path("test", repos.join("invalid/unknown-eapi"), 0, false);
        assert_err_re!(r, "^invalid repo: test: profiles/eapi: unsupported EAPI: unknown$");

        // supported EAPI
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        assert!(EAPIS_OFFICIAL.contains(repo.eapi()));
    }

    #[test]
    fn len() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();

        assert_eq!(temp.len(), 0);
        assert!(temp.is_empty());
        temp.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        assert_eq!(temp.len(), 1);
        assert!(!temp.is_empty());
        temp.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        assert_eq!(temp.len(), 2);
        assert!(!temp.is_empty());
    }

    #[test]
    fn categories() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();

        assert!(temp.categories().is_empty());
        fs::create_dir(temp.path().join("cat")).unwrap();
        assert_ordered_eq!(temp.repo().unwrap().categories(), ["cat"]);
        fs::create_dir(temp.path().join("a-cat")).unwrap();
        fs::create_dir(temp.path().join("z-cat")).unwrap();
        assert_ordered_eq!(temp.repo().unwrap().categories(), ["a-cat", "cat", "z-cat"]);
    }

    #[test]
    fn packages() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();

        assert!(temp.packages("cat").is_empty());
        fs::create_dir_all(temp.path().join("cat/pkg")).unwrap();
        assert_ordered_eq!(temp.repo().unwrap().packages("cat"), ["pkg"]);
        fs::create_dir_all(temp.path().join("a-cat/pkg-z")).unwrap();
        fs::create_dir_all(temp.path().join("a-cat/pkg-a")).unwrap();
        assert_ordered_eq!(temp.repo().unwrap().packages("a-cat"), ["pkg-a", "pkg-z"]);
    }

    #[test]
    fn versions() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        let ver = |s: &str| Version::try_new(s).unwrap();

        assert!(temp.versions("cat", "pkg").is_empty());
        temp.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        assert_ordered_eq!(temp.versions("cat", "pkg"), [ver("1")]);

        // unmatching ebuilds are ignored
        fs::File::create(temp.path().join("cat/pkg/foo-2.ebuild")).unwrap();
        assert_ordered_eq!(temp.versions("cat", "pkg"), [ver("1")]);

        // wrongly named files are ignored
        fs::File::create(temp.path().join("cat/pkg/pkg-2.txt")).unwrap();
        fs::File::create(temp.path().join("cat/pkg/pkg-2..ebuild")).unwrap();
        fs::File::create(temp.path().join("cat/pkg/pkg-2ebuild")).unwrap();
        assert_ordered_eq!(temp.versions("cat", "pkg"), [ver("1")]);

        fs::File::create(temp.path().join("cat/pkg/pkg-2.ebuild")).unwrap();
        assert_ordered_eq!(temp.versions("cat", "pkg"), [ver("1"), ver("2")]);

        fs::create_dir_all(temp.path().join("a-cat/pkg10a")).unwrap();
        fs::File::create(temp.path().join("a-cat/pkg10a/pkg10a-0-r0.ebuild")).unwrap();
        assert_ordered_eq!(temp.versions("a-cat", "pkg10a"), [ver("0-r0")]);
    }

    #[test]
    fn contains() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();

        // path
        assert!(repo.contains(""));
        assert!(!repo.contains("/"));
        assert!(repo.contains(repo.path()));
        assert!(repo.contains("profiles"));
        assert!(!repo.contains("a/pkg"));
        assert!(repo.contains("optional"));
        assert!(repo.contains("optional/none"));
        assert!(repo.contains("optional/none/none-8.ebuild"));
        assert!(!repo.contains("none-8.ebuild"));

        // Cpv
        let cpv = Cpv::try_new("optional/none-8").unwrap();
        assert!(repo.contains(&cpv));
        let cpv = Cpv::try_new("optional/none-0").unwrap();
        assert!(!repo.contains(&cpv));
        let cpv = Cpv::try_new("a/pkg-1").unwrap();
        assert!(!repo.contains(&cpv));

        // Dep
        let d = Dep::try_new("optional/none").unwrap();
        assert!(repo.contains(&d));
        let d = Dep::try_new("=optional/none-8::metadata").unwrap();
        assert!(repo.contains(&d));
        let d = Dep::try_new("=optional/none-0::metadata").unwrap();
        assert!(!repo.contains(&d));
        let d = Dep::try_new("a/pkg").unwrap();
        assert!(!repo.contains(&d));
    }

    #[test]
    fn iter_cpn() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        temp.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkg-1", &[]).unwrap();
        let mut iter = temp.iter_cpn();
        for cpn in ["cat1/pkg", "cat2/pkg"] {
            assert_eq!(iter.next(), Some(Cpn::try_new(cpn).unwrap()));
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn iter_cpn_restrict() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        temp.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkga-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkga-2", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkgb-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkgb-2", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkgb-3", &[]).unwrap();

        // no matches via existing Cpv
        let cpv = Cpv::try_new("cat1/pkga-1").unwrap();
        assert_ordered_eq!(temp.iter_cpn_restrict(&cpv), [] as [Cpn; 0]);

        // no matches via nonexistent Cpv
        let cpv = Cpv::try_new("cat/nonexistent-1").unwrap();
        assert_ordered_eq!(temp.iter_cpn_restrict(&cpv), [] as [Cpn; 0]);

        // single match via Cpn
        let cpn = Cpn::try_new("cat1/pkga").unwrap();
        assert_ordered_eq!(temp.iter_cpn_restrict(&cpn), [cpn]);

        // no matches via Cpn
        let cpn = Cpn::try_new("cat/nonexistent").unwrap();
        assert_ordered_eq!(temp.iter_cpn_restrict(&cpn), [] as [Cpn; 0]);

        // single match via package name
        let restrict = DepRestrict::package("pkgb");
        assert_ordered_eq!(temp.iter_cpn_restrict(restrict).map(|c| c.to_string()), ["cat1/pkgb"]);

        // no matches via package name
        let restrict = DepRestrict::package("nonexistent");
        assert_ordered_eq!(temp.iter_cpn_restrict(restrict), [] as [Cpn; 0]);

        // all Cpns match
        let restrict = Restrict::True;
        assert_ordered_eq!(temp.iter_cpn_restrict(restrict), temp.iter_cpn());

        // no Cpns match
        let restrict = Restrict::False;
        assert_ordered_eq!(temp.iter_cpn_restrict(restrict), [] as [Cpn; 0]);
    }

    #[test]
    fn iter_cpv() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        temp.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkg-1", &[]).unwrap();
        let mut iter = temp.iter_cpv();
        for cpv in ["cat1/pkg-1", "cat2/pkg-1"] {
            assert_eq!(iter.next(), Some(Cpv::try_new(cpv).unwrap()));
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn iter_cpv_restrict() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        temp.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkga-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkga-2", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkgb-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkgb-2", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkgb-3", &[]).unwrap();

        // single match via Cpv
        let cpv = Cpv::try_new("cat1/pkga-1").unwrap();
        assert_ordered_eq!(temp.iter_cpv_restrict(&cpv), [cpv]);

        // no matches via Cpv
        let cpv = Cpv::try_new("cat/nonexistent-1").unwrap();
        assert_ordered_eq!(temp.iter_cpv_restrict(&cpv), []);

        // multiple matches via Cpn
        let cpn = Cpn::try_new("cat1/pkga").unwrap();
        assert_ordered_eq!(
            temp.iter_cpv_restrict(&cpn).map(|c| c.to_string()),
            ["cat1/pkga-1", "cat1/pkga-2"]
        );

        // no matches via Cpn
        let cpn = Cpn::try_new("cat/nonexistent").unwrap();
        assert_ordered_eq!(temp.iter_cpv_restrict(&cpn), []);

        // multiple matches via package name
        let restrict = DepRestrict::package("pkgb");
        assert_ordered_eq!(
            temp.iter_cpv_restrict(restrict).map(|c| c.to_string()),
            ["cat1/pkgb-1", "cat1/pkgb-2", "cat1/pkgb-3"]
        );

        // no matches via package name
        let restrict = DepRestrict::package("nonexistent");
        assert_ordered_eq!(temp.iter_cpv_restrict(restrict), []);

        // all Cpvs match
        let restrict = Restrict::True;
        assert_ordered_eq!(temp.iter_cpv_restrict(restrict), temp.iter_cpv());

        // no Cpvs match
        let restrict = Restrict::False;
        assert_ordered_eq!(temp.iter_cpv_restrict(restrict), []);
    }

    #[test]
    fn iter() {
        let mut config = Config::default();
        let mut temp = config.temp_repo("test", 0, None).unwrap();
        temp.create_raw_pkg("cat2/pkg-1", &[]).unwrap();
        temp.create_raw_pkg("cat1/pkg-1", &[]).unwrap();
        let mut iter = temp.iter();
        for cpv in ["cat1/pkg-1", "cat2/pkg-1"] {
            assert_eq!(iter.next().map(|p| format!("{}", p.cpv())), Some(cpv.to_string()));
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn iter_restrict() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();

        // single match via Cpv
        let cpv = Cpv::try_new("optional/none-8").unwrap();
        assert_ordered_eq!(
            repo.iter_restrict(&cpv).map(|p| p.cpv().to_string()),
            [cpv.to_string()]
        );

        // single match via package
        let pkg = repo.iter().next().unwrap();
        assert_ordered_eq!(
            repo.iter_restrict(&pkg).map(|p| p.cpv().to_string()),
            [pkg.cpv().to_string()],
        );

        // multiple matches via package name
        let restrict = DepRestrict::package("inherit");
        assert!(repo.iter_restrict(restrict).count() > 2);
    }

    #[test]
    fn get_pkg() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();

        // existing
        for cpv in ["slot/slot-8", "slot/subslot-8"] {
            let pkg = repo.get_pkg(cpv).unwrap();
            let raw_pkg = repo.get_pkg_raw(cpv).unwrap();
            assert_eq!(pkg.cpv(), raw_pkg.cpv());
            assert_eq!(pkg.cpv().to_string(), cpv);
        }

        // nonexistent
        assert!(repo.get_pkg("nonexistent/pkg-0").is_err());
        assert!(repo.get_pkg_raw("nonexistent/pkg-0").is_err());

        // invalid Cpv
        assert!(repo.get_pkg("invalid").is_err());
        assert!(repo.get_pkg_raw("invalid-0").is_err());
    }

    #[test]
    fn eclasses() {
        let repo1 = TEST_DATA.ebuild_repo("primary").unwrap();
        assert_ordered_eq!(repo1.eclasses().iter().map(|e| e.name()), ["a", "c"]);
        let repo2 = TEST_DATA.ebuild_repo("secondary").unwrap();
        assert_ordered_eq!(repo2.eclasses().iter().map(|e| e.name()), ["a", "b", "c"]);
        // verify the overridden eclass is from the secondary repo
        let overridden_eclass = repo2.eclasses().get("c").unwrap();
        assert!(overridden_eclass.path().starts_with(repo2.path()));
    }

    #[test]
    fn arches() {
        let repo = TEST_DATA.ebuild_repo("primary").unwrap();
        assert_ordered_eq!(repo.arches(), ["x86"]);
        let repo = TEST_DATA.ebuild_repo("secondary").unwrap();
        assert_ordered_eq!(repo.arches(), ["amd64", "x86"]);
    }

    #[test]
    fn licenses() {
        let repo = TEST_DATA.ebuild_repo("primary").unwrap();
        assert_ordered_eq!(repo.licenses(), ["a"]);
        let repo = TEST_DATA.ebuild_repo("secondary").unwrap();
        assert_ordered_eq!(repo.licenses(), ["a", "b"]);
    }

    #[test]
    fn categories_xml() {
        let repo = TEST_DATA.ebuild_repo("xml").unwrap();
        assert_eq!(repo.categories_xml().get("good").unwrap(), "good");
        // categories with invalid XML data don't have entries
        assert!(repo.categories_xml().get("bad").is_none());
        // categories without XML data don't have entries
        assert!(repo.categories_xml().get("pkg").is_none());
        // nonexistent categories don't have entries
        assert!(repo.categories_xml().get("nonexistent").is_none());
    }
}
