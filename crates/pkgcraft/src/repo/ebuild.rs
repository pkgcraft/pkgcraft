use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock, Weak};
use std::{fmt, fs, iter, mem};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use itertools::{Either, Itertools};
use rayon::prelude::*;
use tracing::warn;

use crate::config::{Config, RepoConfig, Settings};
use crate::dep::{self, Cpn, Cpv, Dep, Operator, Version};
use crate::eapi::Eapi;
use crate::error::Error;
use crate::files::*;
use crate::macros::build_path;
use crate::pkg::ebuild::{EbuildPkg, EbuildRawPkg, keyword::Arch};
use crate::restrict::dep::Restrict as DepRestrict;
use crate::restrict::str::Restrict as StrRestrict;
use crate::restrict::{Restrict, Restriction};
use crate::shell::BuildPool;
use crate::traits::{
    Contains, Intersects, ParallelMap, ParallelMapIter, ParallelMapOrdered,
    ParallelMapOrderedIter,
};
use crate::xml::parse_xml_with_dtd;

use super::{PkgRepository, RepoFormat, Repository, make_repo_traits};

pub mod cache;
pub(crate) mod configured;
mod eclass;
pub use eclass::Eclass;
mod metadata;
pub use metadata::{Metadata, Mirror};
pub mod revdeps;
pub use revdeps::RevDepCache;
mod temp;
pub use temp::{EbuildRepoBuilder, EbuildTempRepo};

struct InternalEbuildRepo {
    metadata: Metadata,
    config: RepoConfig,
    data: LazyMetadata,
}

/// Ebuild repo metadata that is lazily loaded.
#[derive(Default)]
struct LazyMetadata {
    masters: OnceLock<Vec<EbuildRepo>>,
    pool: OnceLock<Weak<BuildPool>>,
    arches: OnceLock<IndexSet<Arch>>,
    licenses: OnceLock<IndexSet<String>>,
    license_groups: OnceLock<IndexMap<String, IndexSet<String>>>,
    mirrors: OnceLock<IndexMap<String, IndexSet<Mirror>>>,
    eclasses: OnceLock<IndexSet<Eclass>>,
    use_expand: OnceLock<IndexMap<String, IndexMap<String, String>>>,
    categories_xml: OnceLock<IndexMap<String, String>>,
}

#[derive(Clone)]
pub struct EbuildRepo(Arc<InternalEbuildRepo>);

impl fmt::Debug for EbuildRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EbuildRepo")
            .field("id", &self.id())
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
    fn from(_repo: &EbuildRepo) -> Self {
        Self::True
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
            priority: Some(priority),
            ..RepoFormat::Ebuild.into()
        };

        Ok(Self(Arc::new(InternalEbuildRepo {
            metadata,
            config,
            data: Default::default(),
        })))
    }

    /// Finalize the repo, resolving repo dependencies and collapsing lazy metadata.
    ///
    /// This collapses lazy fields used in metadata regeneration that leverages
    /// process-based parallelism. Without collapsing, every spawned process reinitializes
    /// any lazy data it accesses, causing significant overhead.
    pub(super) fn finalize(&self, config: &Config) -> crate::Result<()> {
        // check if the repo has already been initialized
        if self.0.data.masters.get().is_some() {
            return Ok(());
        }

        let (masters, repos): (Vec<_>, Vec<_>) =
            self.metadata().config.masters.iter().partition_map(|id| {
                match config.repos.get(id).ok().and_then(|r| r.as_ebuild()) {
                    Some(r) => Either::Left(r.clone()),
                    None => Either::Right(id.to_string()),
                }
            });

        if !repos.is_empty() {
            return Err(Error::NonexistentRepoMasters { repos });
        }

        self.0
            .data
            .masters
            .set(masters)
            .unwrap_or_else(|_| panic!("re-finalizing repo: {self}"));

        self.0
            .data
            .pool
            .set(Arc::downgrade(config.pool()))
            .unwrap_or_else(|_| panic!("re-finalizing repo: {self}"));

        // collapse lazy fields
        self.eclasses();
        self.arches();
        self.licenses();

        Ok(())
    }

    /// Return the repo config.
    pub(super) fn repo_config(&self) -> &RepoConfig {
        &self.0.config
    }

    /// Return the repo's path.
    pub fn path(&self) -> &Utf8Path {
        &self.repo_config().location
    }

    /// Return the build pool for the repo.
    pub fn pool(&self) -> Arc<BuildPool> {
        self.0
            .data
            .pool
            .get()
            .unwrap_or_else(|| panic!("uninitialized ebuild repo: {self}"))
            .upgrade()
            .unwrap_or_else(|| panic!("destroyed ebuild repo: {self}"))
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
        self.0
            .data
            .masters
            .get()
            .unwrap_or_else(|| panic!("uninitialized ebuild repo: {self}"))
    }

    /// Return the iterator of all inherited repos.
    pub fn trees(&self) -> impl Iterator<Item = &Self> {
        [self].into_iter().chain(self.masters())
    }

    /// Return the ordered map of inherited eclasses.
    pub fn eclasses(&self) -> &IndexSet<Eclass> {
        self.0.data.eclasses.get_or_init(|| {
            let mut eclasses: IndexSet<_> = self
                .trees()
                .flat_map(|r| r.metadata().eclasses().clone())
                .collect();
            eclasses.sort_unstable();
            eclasses
        })
    }

    /// Return the ordered map of inherited USE_EXPAND flags.
    pub fn use_expand(&self) -> &IndexMap<String, IndexMap<String, String>> {
        self.0.data.use_expand.get_or_init(|| {
            let mut use_expand: IndexMap<_, _> = self
                .trees()
                .flat_map(|r| r.metadata().use_expand().clone())
                .collect();
            use_expand.sort_unstable_keys();
            use_expand
        })
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

        self.0.data.categories_xml.get_or_init(|| {
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

    /// Try to convert an ebuild file path into a Cpv.
    fn cpv_from_path(&self, path: &Utf8Path) -> crate::Result<Cpv> {
        let relpath = path.strip_prefix(self.path()).unwrap_or(path);
        let path_err = |s: &str| -> Error {
            Error::InvalidValue(format!("invalid ebuild path: {relpath}: {s}"))
        };
        let (cat, pkg, file) = relpath
            .components()
            .map(|s| s.as_str())
            .collect_tuple()
            .ok_or_else(|| path_err("mismatched path components"))?;
        let cpn = Cpn::try_from((cat, pkg))?;
        let p = file
            .strip_suffix(".ebuild")
            .ok_or_else(|| path_err("missing ebuild ext"))?;
        let version = p
            .strip_prefix(cpn.package())
            .and_then(|s| s.strip_prefix('-'));
        let Some(version) = version else {
            if p.contains('-') {
                return Err(Error::InvalidValue(format!("{file}: mismatched package name")));
            } else {
                return Err(Error::InvalidValue(format!("{file}: missing version")));
            }
        };
        let version = Version::try_new(version)
            .map_err(|_| Error::InvalidValue(format!("{file}: invalid version: {version}")))?;
        Ok(Cpv { cpn, version })
    }

    /// Return the set of inherited architectures sorted by name.
    pub fn arches(&self) -> &IndexSet<Arch> {
        self.0.data.arches.get_or_init(|| {
            let mut arches: IndexSet<_> = self
                .trees()
                .flat_map(|r| r.metadata().arches().clone())
                .collect();
            arches.sort_unstable();
            arches
        })
    }

    /// Return the set of inherited licenses sorted by name.
    pub fn licenses(&self) -> &IndexSet<String> {
        self.0.data.licenses.get_or_init(|| {
            let mut licenses: IndexSet<_> = self
                .trees()
                .flat_map(|r| r.metadata().licenses().clone())
                .collect();
            licenses.sort_unstable();
            licenses
        })
    }

    /// Return the mapping of license groups merged via inheritance.
    pub fn license_groups(&self) -> &IndexMap<String, IndexSet<String>> {
        self.0.data.license_groups.get_or_init(|| {
            let mut license_groups: IndexMap<_, _> = self
                .trees()
                .flat_map(|r| r.metadata().license_groups().clone())
                .collect();
            license_groups.sort_keys();
            license_groups
        })
    }

    /// Return the set of mirrors merged via inheritance.
    pub fn mirrors(&self) -> &IndexMap<String, IndexSet<Mirror>> {
        self.0.data.mirrors.get_or_init(|| {
            let mut mirrors: IndexMap<_, _> = self
                .trees()
                .flat_map(|r| r.metadata().mirrors().clone())
                .collect();
            mirrors.sort_keys();
            mirrors
        })
    }

    /// Return the sorted set of Cpvs from a given category.
    pub fn cpvs_from_category(&self, category: &str) -> IndexSet<Cpv> {
        let path = build_path!(self.path(), category);
        if let Ok(entries) = path.read_dir_utf8() {
            let mut cpvs: IndexSet<_> = entries
                .filter_map(Result::ok)
                .flat_map(|e| self.cpvs_from_package(category, e.file_name()))
                .filter_map(Result::ok)
                .collect();
            cpvs.sort_unstable();
            cpvs
        } else {
            Default::default()
        }
    }

    /// Return the sorted iterator of Cpv results for a package.
    ///
    /// These are constructed from the raw *.ebuild file names in the package directory,
    /// returning errors when invalid.
    pub fn cpvs_from_package(
        &self,
        category: &str,
        package: &str,
    ) -> impl Iterator<Item = crate::Result<Cpv>> + use<> {
        let path = build_path!(self.path(), category, package);
        if let Ok(entries) = path.read_dir_utf8() {
            let cpvs: Vec<_> = entries
                .filter_map(Result::ok)
                .filter(is_ebuild)
                .map(|e| self.cpv_from_path(e.path()))
                .sorted()
                .collect();
            Either::Left(cpvs.into_iter())
        } else {
            Either::Right(std::iter::empty())
        }
    }

    /// Return an ordered iterator of ebuild packages for the repo.
    ///
    /// This constructs packages in parallel and returns them in repo order.
    pub fn iter_ordered(&self) -> IterOrdered {
        IterOrdered::new(self, None)
    }

    /// Return an unordered iterator of ebuild packages for the repo.
    ///
    /// This constructs packages in parallel and returns them in completion order.
    pub fn iter_unordered(&self) -> IterUnordered {
        IterUnordered::new(self, None)
    }

    /// Return an ordered iterator of ebuild packages for the repo matching a given
    /// restriction.
    ///
    /// This constructs packages in parallel and returns them in repo order.
    pub fn iter_restrict_ordered<R: Into<Restrict>>(&self, value: R) -> IterRestrictOrdered {
        IterRestrictOrdered::new(self, value)
    }

    /// Return an iterator of raw packages for the repo.
    pub fn iter_raw(&self) -> IterRaw {
        IterRaw::new(self, None)
    }

    /// Return an ordered iterator of raw packages for the repo.
    ///
    /// This constructs packages in parallel and returns them in repo order.
    pub fn iter_raw_ordered(&self) -> IterRawOrdered {
        IterRawOrdered::new(self, None)
    }

    /// Return a filtered iterator of raw packages for the repo.
    pub fn iter_raw_restrict<R: Into<Restrict>>(&self, value: R) -> IterRawRestrict {
        IterRawRestrict::new(self, value)
    }

    /// Return an ordered iterator of raw packages for the repo matching a given
    /// restriction.
    ///
    /// This constructs packages in parallel and returns them in repo order.
    pub fn iter_raw_restrict_ordered<R: Into<Restrict>>(
        &self,
        value: R,
    ) -> IterRawRestrictOrdered {
        IterRawRestrictOrdered::new(self, value)
    }

    /// Retrieve a package from the repo given its [`Cpv`].
    pub fn get_pkg<T>(&self, value: T) -> crate::Result<EbuildPkg>
    where
        T: TryInto<Cpv>,
        Error: From<T::Error>,
    {
        let raw_pkg = self.get_pkg_raw(value)?;
        raw_pkg.try_into()
    }

    /// Retrieve a raw package from the repo given its [`Cpv`].
    pub fn get_pkg_raw<T>(&self, value: T) -> crate::Result<EbuildRawPkg>
    where
        T: TryInto<Cpv>,
        Error: From<T::Error>,
    {
        let cpv = value.try_into()?;
        EbuildRawPkg::try_new(cpv, self)
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
    pub fn configure<T: Into<Arc<Settings>>>(
        &self,
        settings: T,
    ) -> configured::ConfiguredRepo {
        configured::ConfiguredRepo::new(self.clone(), settings.into())
    }

    /// Return the RevDepCache for the repo.
    pub fn revdeps(&self, ignore: bool) -> crate::Result<RevDepCache> {
        RevDepCache::from_repo(self, ignore)
    }
}

impl fmt::Display for EbuildRepo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl PkgRepository for EbuildRepo {
    type Pkg = EbuildPkg;
    type IterCpn = IterCpn;
    type IterCpnRestrict = IterCpnRestrict;
    type IterCpv = IterCpv;
    type IterCpvRestrict = IterCpvRestrict;
    type Iter = Iter;
    type IterRestrict = IterRestrict;

    fn categories(&self) -> IndexSet<String> {
        let mut categories: IndexSet<_> = self
            .trees()
            .flat_map(|r| r.metadata().categories())
            .filter(|x| self.path().join(x).is_dir())
            .cloned()
            .collect();
        categories.sort_unstable();
        categories
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
            .filter(is_ebuild)
            .filter_map(|entry| {
                let p = entry.path().file_stem().expect("invalid ebuild file");
                p.strip_prefix(pkg)
                    .and_then(|s| s.strip_prefix('-'))
                    .and_then(|s| Version::try_new(s).ok())
            })
            .collect();
        versions.sort_unstable();
        versions
    }

    fn iter_cpn(&self) -> IterCpn {
        IterCpn::new(self, None)
    }

    fn iter_cpn_restrict<R: Into<Restrict>>(&self, value: R) -> IterCpnRestrict {
        IterCpnRestrict::new(self, value)
    }

    fn iter_cpv(&self) -> IterCpv {
        IterCpv::new(self, None)
    }

    fn iter_cpv_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterCpvRestrict {
        IterCpvRestrict::new(self, value)
    }

    fn iter(&self) -> Self::Iter {
        self.into_iter()
    }

    fn iter_restrict<R: Into<Restrict>>(&self, value: R) -> Self::IterRestrict {
        IterRestrict::new(self, value)
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
        self.repo_config().priority()
    }

    fn path(&self) -> &Utf8Path {
        self.path()
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
    type Item = crate::Result<EbuildPkg>;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        Iter::new(self, None)
    }
}

/// Ordered iterable of results from constructing ebuild packages.
pub struct Iter(IterRaw);

impl Iter {
    fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        Self(IterRaw::new(repo, restrict))
    }
}

impl Iterator for Iter {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|r| r.and_then(|raw_pkg| raw_pkg.try_into()))
    }
}

/// Unordered iterable of results from constructing ebuild packages.
///
/// This constructs packages in parallel and returns them as completed.
pub struct IterUnordered {
    iter: ParallelMapIter<crate::Result<EbuildPkg>>,
}

impl IterUnordered {
    fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        let pkgs = IterRaw::new(repo, restrict);
        let func =
            move |result: crate::Result<EbuildRawPkg>| result.and_then(|pkg| pkg.try_into());
        Self {
            iter: pkgs.par_map(func).into_iter(),
        }
    }
}

impl Iterator for IterUnordered {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Ordered iterable of results from constructing ebuild packages.
///
/// This constructs packages in parallel and returns them in repo order.
pub struct IterOrdered {
    iter: ParallelMapOrderedIter<crate::Result<EbuildPkg>>,
}

impl IterOrdered {
    fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        let pkgs = IterRaw::new(repo, restrict);
        let func =
            move |result: crate::Result<EbuildRawPkg>| result.and_then(|pkg| pkg.try_into());
        Self {
            iter: pkgs.par_map_ordered(func).into_iter(),
        }
    }
}

impl Iterator for IterOrdered {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
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
            iter: IterCpv::new(repo, restrict),
            repo: repo.clone(),
        }
    }
}

impl Iterator for IterRaw {
    type Item = crate::Result<EbuildRawPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|cpv| EbuildRawPkg::try_new(cpv, &self.repo))
    }
}

/// Iterable of [`Cpn`] objects.
pub struct IterCpn(Box<dyn Iterator<Item = Cpn> + Send>);

impl IterCpn {
    fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        use DepRestrict::{Category, Package};
        use StrRestrict::Equal;
        let mut cat_restricts = vec![];
        let mut pkg_restricts = vec![];
        let repo = repo.clone();

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
                let cat = mem::take(cat);
                let pn = mem::take(pn);
                if let Ok(cpn) = Cpn::try_from((cat, pn)) {
                    if repo.contains(&cpn) {
                        Box::new(iter::once(cpn))
                    } else {
                        Box::new(iter::empty())
                    }
                } else {
                    Box::new(iter::empty())
                }
            }
            ([Equal(cat)], _) => {
                let cat = mem::take(cat);
                let pkg_restrict = Restrict::and(pkg_restricts);
                Box::new(repo.packages(&cat).into_iter().filter_map(move |pn| {
                    if pkg_restrict.matches(&pn) {
                        Some(Cpn {
                            category: cat.clone(),
                            package: pn,
                        })
                    } else {
                        None
                    }
                }))
            }
            (_, [Equal(pn)]) => {
                let pn = mem::take(pn);
                let cat_restrict = Restrict::and(cat_restricts);
                Box::new(repo.categories().into_iter().filter_map(move |cat| {
                    if cat_restrict.matches(&cat) {
                        let cpn = Cpn {
                            category: cat,
                            package: pn.clone(),
                        };
                        if repo.contains(&cpn) {
                            return Some(cpn);
                        }
                    }
                    None
                }))
            }
            _ => {
                let cat_restrict = Restrict::and(cat_restricts);
                let pkg_restrict = Restrict::and(pkg_restricts);
                Box::new(
                    repo.categories()
                        .into_iter()
                        .filter(move |cat| cat_restrict.matches(cat))
                        .flat_map(move |cat| {
                            repo.packages(&cat)
                                .into_iter()
                                .filter(|pn| pkg_restrict.matches(pn))
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
pub struct IterCpv(Box<dyn Iterator<Item = Cpv> + Send>);

impl IterCpv {
    fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        use DepRestrict::{Category, Package, Version};
        use StrRestrict::Equal;
        let mut cat_restricts = vec![];
        let mut pkg_restricts = vec![];
        let mut ver_restricts = vec![];
        let repo = repo.clone();

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
                if let Ok(cpv) = Cpv::try_from((cat, pn, ver.without_op())) {
                    if repo.contains(&cpv) {
                        Box::new(iter::once(cpv))
                    } else {
                        Box::new(iter::empty())
                    }
                } else {
                    Box::new(iter::empty())
                }
            }
            ([Category(Equal(cat))], [Package(Equal(pn))], _) => {
                let ver_restrict = Restrict::and(ver_restricts);
                Box::new(
                    repo.cpvs_from_package(cat, pn)
                        .filter_map(Result::ok)
                        .filter(move |cpv| ver_restrict.matches(cpv)),
                )
            }
            ([], [Package(Equal(pn))], _) => {
                let pn = mem::take(pn);
                let ver_restrict = Restrict::and(ver_restricts);
                Box::new(repo.categories().into_iter().flat_map(move |cat| {
                    repo.cpvs_from_package(&cat, &pn)
                        .filter_map(Result::ok)
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
                        .filter(move |s| cat_restrict.matches(s))
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
    iter: Either<iter::Empty<<Iter as Iterator>::Item>, Iter>,
    restrict: Restrict,
}

impl IterRestrict {
    fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = if restrict == Restrict::False {
            Either::Left(iter::empty())
        } else {
            Either::Right(Iter::new(repo, Some(&restrict)))
        };
        Self { iter, restrict }
    }
}

impl Iterator for IterRestrict {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|r| match r {
            Ok(pkg) if self.restrict.matches(&pkg) => Some(Ok(pkg)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

/// Ordered iterable of results from constructing ebuild packages matching a given
/// restriction.
///
/// This constructs packages in parallel and returns them in repo order.
pub struct IterRestrictOrdered {
    iter: Either<iter::Empty<<IterOrdered as Iterator>::Item>, IterOrdered>,
    restrict: Restrict,
}

impl IterRestrictOrdered {
    fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = if restrict == Restrict::False {
            Either::Left(iter::empty())
        } else {
            Either::Right(IterOrdered::new(repo, Some(&restrict)))
        };
        Self { iter, restrict }
    }
}

impl Iterator for IterRestrictOrdered {
    type Item = crate::Result<EbuildPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|r| match r {
            Ok(pkg) if self.restrict.matches(&pkg) => Some(Ok(pkg)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

/// Iterable of [`Cpn`] objects matching a given restriction.
pub struct IterCpnRestrict {
    iter: Either<iter::Empty<<IterCpn as Iterator>::Item>, IterCpn>,
    restrict: Restrict,
}

impl IterCpnRestrict {
    fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = if restrict == Restrict::False {
            Either::Left(iter::empty())
        } else {
            Either::Right(IterCpn::new(repo, Some(&restrict)))
        };
        Self { iter, restrict }
    }
}

impl Iterator for IterCpnRestrict {
    type Item = Cpn;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|cpn| self.restrict.matches(cpn))
    }
}

/// Iterable of [`Cpv`] objects matching a given restriction.
pub struct IterCpvRestrict {
    iter: Either<iter::Empty<<IterCpv as Iterator>::Item>, IterCpv>,
    restrict: Restrict,
}

impl IterCpvRestrict {
    fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = if restrict == Restrict::False {
            Either::Left(iter::empty())
        } else {
            Either::Right(IterCpv::new(repo, Some(&restrict)))
        };
        Self { iter, restrict }
    }
}

impl Iterator for IterCpvRestrict {
    type Item = Cpv;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find(|cpv| self.restrict.matches(cpv))
    }
}

/// Iterable of valid, raw ebuild packages matching a given restriction.
pub struct IterRawRestrict {
    iter: Either<iter::Empty<<IterRaw as Iterator>::Item>, IterRaw>,
    restrict: Restrict,
}

impl IterRawRestrict {
    fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = if restrict == Restrict::False {
            Either::Left(iter::empty())
        } else {
            Either::Right(IterRaw::new(repo, Some(&restrict)))
        };
        Self { iter, restrict }
    }
}

impl Iterator for IterRawRestrict {
    type Item = crate::Result<EbuildRawPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|r| match r {
            Ok(pkg) if self.restrict.matches(&pkg) => Some(Ok(pkg)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

/// Ordered iterable of results from constructing raw packages.
///
/// This constructs packages in parallel and returns them in repo order.
pub struct IterRawOrdered {
    iter: ParallelMapOrderedIter<crate::Result<EbuildRawPkg>>,
}

impl IterRawOrdered {
    fn new(repo: &EbuildRepo, restrict: Option<&Restrict>) -> Self {
        let cpvs = IterCpv::new(repo, restrict);
        let repo = repo.clone();
        let func = move |cpv: Cpv| repo.get_pkg_raw(cpv);
        Self {
            iter: cpvs.par_map_ordered(func).into_iter(),
        }
    }
}

impl Iterator for IterRawOrdered {
    type Item = crate::Result<EbuildRawPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Ordered iterable of results from constructing raw packages matching a given
/// restriction.
///
/// This constructs packages in parallel and returns them in repo order.
pub struct IterRawRestrictOrdered {
    iter: Either<iter::Empty<<IterRawOrdered as Iterator>::Item>, IterRawOrdered>,
    restrict: Restrict,
}

impl IterRawRestrictOrdered {
    fn new<R: Into<Restrict>>(repo: &EbuildRepo, value: R) -> Self {
        let restrict = value.into();
        let iter = if restrict == Restrict::False {
            Either::Left(iter::empty())
        } else {
            Either::Right(IterRawOrdered::new(repo, Some(&restrict)))
        };
        Self { iter, restrict }
    }
}

impl Iterator for IterRawRestrictOrdered {
    type Item = crate::Result<EbuildRawPkg>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.find_map(|r| match r {
            Ok(pkg) if self.restrict.matches(&pkg) => Some(Ok(pkg)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::eapi::EAPIS_OFFICIAL;
    use crate::pkg::Package;
    use crate::test::*;

    use super::*;

    #[test]
    fn masters() {
        let data = test_data();
        let repos = data.path().join("repos");

        // none
        let mut config = Config::default();
        let repo = config
            .add_repo_path("a", repos.join("valid/primary"), 0)
            .unwrap();
        config.finalize().unwrap();
        let primary_repo = repo.as_ebuild().unwrap();
        assert!(primary_repo.masters().is_empty());
        assert_ordered_eq!(primary_repo.trees(), [primary_repo]);

        // nonexistent
        let mut config = Config::default();
        config
            .add_repo_path("primary", repos.join("valid/primary"), 0)
            .unwrap();
        let r = config.add_repo_path("test", repos.join("invalid/nonexistent-masters"), 0);
        assert_err_re!(r, "^nonexistent masters: nonexistent1, nonexistent2$");

        // single
        let mut config = Config::default();
        let r1 = config
            .add_repo_path("primary", repos.join("valid/primary"), 0)
            .unwrap();
        let r2 = config
            .add_repo_path("secondary", repos.join("valid/secondary"), 0)
            .unwrap();
        config.finalize().unwrap();
        let primary_repo = r1.as_ebuild().unwrap();
        let secondary_repo = r2.as_ebuild().unwrap();
        assert_ordered_eq!(secondary_repo.masters(), [primary_repo]);
        assert_ordered_eq!(secondary_repo.trees(), [secondary_repo, primary_repo]);
    }

    #[test]
    fn invalid() {
        let data = test_data();
        let repos = data.path().join("repos");

        // invalid profiles/eapi file
        let path = repos.join("invalid/invalid-eapi");
        let r = EbuildRepo::from_path(&path, 0, &path);
        assert_err_re!(
            r,
            format!(
                r##"^invalid repo: {path}: profiles/eapi: invalid EAPI: "# invalid\\n8""##
            )
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
        let data = test_data();

        // repo id matches name
        let repo = data.ebuild_repo("primary").unwrap();
        assert_eq!(repo.id(), "primary");
        assert_eq!(repo.name(), "primary");
        assert!(format!("{repo:?}").contains(repo.id()));

        // repo id differs from name
        let repo = EbuildRepo::from_path("name", 0, repo.path()).unwrap();
        assert_eq!(repo.id(), "name");
        assert_eq!(repo.name(), "primary");
        assert!(!format!("{repo:?}").contains(repo.name()));
    }

    #[test]
    fn restrict_from_path() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        fs::File::create(temp.path().join("cat/pkg/pkga-1.ebuild")).unwrap();
        fs::File::create(temp.path().join("cat/pkg/pkg.ebuild")).unwrap();
        fs::create_dir_all(temp.path().join("cat/pkg/files")).unwrap();
        config.finalize().unwrap();

        // non-repo path
        assert!(repo.restrict_from_path("/").is_none());

        // nonexistent path
        assert!(
            repo.restrict_from_path(repo.path().join("/nonexistent/path"))
                .is_none()
        );

        // repo root
        assert_eq!(repo.restrict_from_path(repo.path()).unwrap(), Restrict::True);

        // non-package path
        assert_eq!(repo.restrict_from_path("profiles").unwrap(), Restrict::False);

        // nonexistent repo path
        assert!(repo.restrict_from_path("cat/pkg/pkg-0.ebuild").is_none());

        // category dir
        assert_eq!(
            repo.restrict_from_path("cat").unwrap(),
            Restrict::and([DepRestrict::category("cat")])
        );

        // package dir
        assert_eq!(
            repo.restrict_from_path("cat/pkg").unwrap(),
            Restrict::and([DepRestrict::category("cat"), DepRestrict::package("pkg")])
        );

        // ebuild file
        assert_eq!(
            repo.restrict_from_path("cat/pkg/pkg-1.ebuild").unwrap(),
            Restrict::and([
                DepRestrict::category("cat"),
                DepRestrict::package("pkg"),
                DepRestrict::version("1").unwrap()
            ])
        );

        // ebuild with invalid file name
        assert_eq!(repo.restrict_from_path("cat/pkg/pkga-1.ebuild").unwrap(), Restrict::False);

        // ebuild with invalid file name
        assert_eq!(repo.restrict_from_path("cat/pkg/pkg.ebuild").unwrap(), Restrict::False);

        // non-ebuild package path
        assert_eq!(repo.restrict_from_path("cat/pkg/files").unwrap(), Restrict::False);
    }

    #[test]
    fn eapi() {
        let data = test_data();
        let mut config = Config::default();
        let repos = data.path().join("repos");

        // nonexistent profiles/eapi file uses EAPI 0 which isn't supported
        let r = config.add_repo_path("test", repos.join("invalid/unsupported-eapi"), 0);
        assert_err_re!(r, "^invalid repo: test: profiles/eapi: unsupported EAPI: 0$");

        // unknown EAPI
        let r = config.add_repo_path("test", repos.join("invalid/unknown-eapi"), 0);
        assert_err_re!(r, "^invalid repo: test: profiles/eapi: unsupported EAPI: unknown$");

        // supported EAPI
        let repo = data.ebuild_repo("metadata").unwrap();
        assert!(EAPIS_OFFICIAL.contains(repo.eapi()));
    }

    #[test]
    fn len() {
        let data = test_data();
        let repo = data.ebuild_repo("empty").unwrap();
        assert_eq!(repo.len(), 0);
        assert!(repo.is_empty());

        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        temp.create_ebuild("cat2/pkg-1", &[]).unwrap();
        config.finalize().unwrap();

        assert_eq!(repo.len(), 2);
        assert!(!repo.is_empty());
    }

    #[test]
    fn categories() {
        let data = test_data();
        let repo = data.ebuild_repo("empty").unwrap();
        assert!(repo.categories().is_empty());

        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        temp.create_ebuild("a-cat/pkg-1", &[]).unwrap();
        temp.create_ebuild("z-cat/pkg-1", &[]).unwrap();
        config.finalize().unwrap();

        assert_ordered_eq!(repo.categories(), ["a-cat", "cat", "z-cat"]);
    }

    #[test]
    fn packages() {
        let mut config = Config::default();
        let temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        assert!(repo.packages("cat").is_empty());
        fs::create_dir_all(temp.path().join("cat/pkg")).unwrap();
        assert_ordered_eq!(repo.packages("cat"), ["pkg"]);
        fs::create_dir_all(temp.path().join("a-cat/pkg-z")).unwrap();
        fs::create_dir_all(temp.path().join("a-cat/pkg-a")).unwrap();
        assert_ordered_eq!(repo.packages("a-cat"), ["pkg-a", "pkg-z"]);
    }

    #[test]
    fn versions() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        let ver = |s: &str| Version::try_new(s).unwrap();

        assert!(repo.versions("cat", "pkg").is_empty());
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        assert_ordered_eq!(repo.versions("cat", "pkg"), [ver("1")]);

        // unmatching ebuilds are ignored
        fs::File::create(temp.path().join("cat/pkg/foo-2.ebuild")).unwrap();
        assert_ordered_eq!(repo.versions("cat", "pkg"), [ver("1")]);

        // wrongly named files are ignored
        fs::File::create(temp.path().join("cat/pkg/pkg-2.txt")).unwrap();
        fs::File::create(temp.path().join("cat/pkg/pkg-2..ebuild")).unwrap();
        fs::File::create(temp.path().join("cat/pkg/pkg-2ebuild")).unwrap();
        assert_ordered_eq!(repo.versions("cat", "pkg"), [ver("1")]);

        fs::File::create(temp.path().join("cat/pkg/pkg-2.ebuild")).unwrap();
        assert_ordered_eq!(repo.versions("cat", "pkg"), [ver("1"), ver("2")]);

        fs::create_dir_all(temp.path().join("a-cat/pkg10a")).unwrap();
        fs::File::create(temp.path().join("a-cat/pkg10a/pkg10a-0-r0.ebuild")).unwrap();
        assert_ordered_eq!(repo.versions("a-cat", "pkg10a"), [ver("0-r0")]);
    }

    #[test]
    fn contains() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        temp.create_ebuild("cat/pkg-2", &[]).unwrap();
        config.finalize().unwrap();

        // path
        assert!(repo.contains(""));
        assert!(!repo.contains("/"));
        assert!(repo.contains(repo.path()));
        assert!(repo.contains("profiles"));
        assert!(!repo.contains("a/pkg"));
        assert!(repo.contains("cat"));
        assert!(repo.contains("cat/pkg"));
        assert!(repo.contains("cat/pkg/pkg-1.ebuild"));
        assert!(!repo.contains("pkg-1.ebuild"));

        // Cpn
        let cpn = Cpn::try_new("cat/pkg").unwrap();
        assert!(repo.contains(&cpn));
        let cpn = Cpn::try_new("a/pkg").unwrap();
        assert!(!repo.contains(&cpn));

        // Cpv
        let cpv = Cpv::try_new("cat/pkg-1").unwrap();
        assert!(repo.contains(&cpv));
        let cpv = Cpv::try_new("cat/pkg-0").unwrap();
        assert!(!repo.contains(&cpv));
        let cpv = Cpv::try_new("a/pkg-1").unwrap();
        assert!(!repo.contains(&cpv));

        // Dep
        let d = Dep::try_new("cat/pkg").unwrap();
        assert!(repo.contains(&d));
        let d = Dep::try_new("=cat/pkg-1").unwrap();
        assert!(repo.contains(&d));
        let d = Dep::try_new("=cat/pkg-1::test").unwrap();
        assert!(repo.contains(&d));
        let d = Dep::try_new("=cat/pkg-0").unwrap();
        assert!(!repo.contains(&d));
        let d = Dep::try_new("=cat/pkg-1::repo").unwrap();
        assert!(!repo.contains(&d));
        let d = Dep::try_new("a/pkg").unwrap();
        assert!(!repo.contains(&d));

        // Restrict
        assert!(repo.contains(&Restrict::True));
        assert!(!repo.contains(&Restrict::False));
        let restrict = Restrict::from(Cpn::try_new("cat/pkg").unwrap());
        assert!(repo.contains(&restrict));
        let restrict = Restrict::from(Cpv::try_new("cat/pkg-1").unwrap());
        assert!(repo.contains(&restrict));
    }

    #[test]
    fn iter_cpn() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat2/pkg-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkg-1", &[]).unwrap();

        let mut iter = repo.iter_cpn();
        for cpn in ["cat1/pkg", "cat2/pkg"] {
            assert_eq!(iter.next(), Some(Cpn::try_new(cpn).unwrap()));
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn iter_cpn_restrict() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat2/pkg-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkga-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkga-2", &[]).unwrap();
        temp.create_ebuild("cat1/pkgb-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkgb-2", &[]).unwrap();
        temp.create_ebuild("cat1/pkgb-3", &[]).unwrap();

        // matching restriction
        let restrict = Restrict::True;
        assert_ordered_eq!(repo.iter_cpn_restrict(restrict), repo.iter_cpn());

        // non-matching restriction
        let restrict = Restrict::False;
        assert!(repo.iter_cpn_restrict(restrict).next().is_none());

        // no matches via existing Cpv
        let cpv = Cpv::try_new("cat1/pkga-1").unwrap();
        assert_ordered_eq!(repo.iter_cpn_restrict(&cpv), [] as [Cpn; 0]);

        // no matches via nonexistent Cpv
        let cpv = Cpv::try_new("cat/nonexistent-1").unwrap();
        assert_ordered_eq!(repo.iter_cpn_restrict(&cpv), [] as [Cpn; 0]);

        // single match via Cpn
        let cpn = Cpn::try_new("cat1/pkga").unwrap();
        assert_ordered_eq!(repo.iter_cpn_restrict(&cpn), [cpn]);

        // no matches via Cpn
        let cpn = Cpn::try_new("cat/nonexistent").unwrap();
        assert_ordered_eq!(repo.iter_cpn_restrict(&cpn), [] as [Cpn; 0]);

        // invalid Cpn restrict
        let cat_restrict = DepRestrict::category("-cat");
        let pn_restrict = DepRestrict::package("-pkg");
        let restrict = Restrict::and([cat_restrict, pn_restrict]);
        assert_ordered_eq!(repo.iter_cpn_restrict(restrict), [] as [Cpn; 0]);

        // single match via package name
        let restrict = DepRestrict::package("pkgb");
        assert_ordered_eq!(
            repo.iter_cpn_restrict(restrict).map(|c| c.to_string()),
            ["cat1/pkgb"]
        );

        // no matches via package name
        let restrict = DepRestrict::package("nonexistent");
        assert_ordered_eq!(repo.iter_cpn_restrict(restrict), [] as [Cpn; 0]);

        // no matches via globbed dep restrict
        let restrict = crate::restrict::parse::dep("cat1/*z").unwrap();
        assert_ordered_eq!(repo.iter_cpn_restrict(restrict), [] as [Cpn; 0]);

        // matches via globbed dep restrict
        let restrict = crate::restrict::parse::dep("*1/*a").unwrap();
        let cpn = Cpn::try_new("cat1/pkga").unwrap();
        assert_ordered_eq!(repo.iter_cpn_restrict(restrict), [cpn]);
    }

    #[test]
    fn iter_cpv() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat2/pkg-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkg-1", &[]).unwrap();
        let mut iter = repo.iter_cpv();
        for cpv in ["cat1/pkg-1", "cat2/pkg-1"] {
            assert_eq!(iter.next(), Some(Cpv::try_new(cpv).unwrap()));
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn iter_cpv_restrict() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat2/pkg-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkga-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkga-2", &[]).unwrap();
        temp.create_ebuild("cat1/pkgb-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkgb-2", &[]).unwrap();
        temp.create_ebuild("cat1/pkgb-3", &[]).unwrap();

        // matching restriction
        let restrict = Restrict::True;
        assert_ordered_eq!(repo.iter_cpv_restrict(restrict), repo.iter_cpv());

        // non-matching restriction
        let restrict = Restrict::False;
        assert!(repo.iter_cpv_restrict(restrict).next().is_none());

        // single match via Cpv
        let cpv = Cpv::try_new("cat1/pkga-1").unwrap();
        assert_ordered_eq!(repo.iter_cpv_restrict(&cpv), [cpv]);

        // no matches via Cpv
        let cpv = Cpv::try_new("cat/nonexistent-1").unwrap();
        assert_ordered_eq!(repo.iter_cpv_restrict(&cpv), []);

        // invalid Cpv restrict
        let cat_restrict = DepRestrict::category("-cat");
        let pn_restrict = DepRestrict::package("-pkg");
        let ver_restrict = DepRestrict::version("1").unwrap();
        let restrict = Restrict::and([cat_restrict, pn_restrict, ver_restrict]);
        assert_ordered_eq!(repo.iter_cpv_restrict(restrict), [] as [Cpv; 0]);

        // multiple matches via Cpn
        let cpn = Cpn::try_new("cat1/pkga").unwrap();
        assert_ordered_eq!(
            repo.iter_cpv_restrict(&cpn).map(|c| c.to_string()),
            ["cat1/pkga-1", "cat1/pkga-2"]
        );

        // no matches via Cpn
        let cpn = Cpn::try_new("cat/nonexistent").unwrap();
        assert_ordered_eq!(repo.iter_cpv_restrict(&cpn), []);

        // multiple matches via package name
        let restrict = DepRestrict::package("pkgb");
        assert_ordered_eq!(
            repo.iter_cpv_restrict(restrict).map(|c| c.to_string()),
            ["cat1/pkgb-1", "cat1/pkgb-2", "cat1/pkgb-3"]
        );

        // no matches via package name
        let restrict = DepRestrict::package("nonexistent");
        assert_ordered_eq!(repo.iter_cpv_restrict(restrict), []);
    }

    #[test]
    fn iter() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat2/pkg-1", &[]).unwrap();
        temp.create_ebuild("cat1/pkg-1", &[]).unwrap();
        let pkgs: Vec<_> = repo.iter().try_collect().unwrap();
        assert_ordered_eq!(
            pkgs.iter().map(|x| x.cpv().to_string()),
            ["cat1/pkg-1", "cat2/pkg-1"]
        );
    }

    #[test]
    fn iter_restrict() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // non-matching restriction
        let restrict = Restrict::False;
        assert!(repo.iter_restrict(restrict).next().is_none());

        // single match via Cpv
        let cpv = Cpv::try_new("optional/none-8").unwrap();
        let pkgs: Vec<_> = repo.iter_restrict(&cpv).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), [cpv.to_string()]);

        // single match via package
        let pkg = repo.iter().next().unwrap().unwrap();
        let pkgs: Vec<_> = repo.iter_restrict(&pkg).try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|p| p.cpv().to_string()), [pkg.cpv().to_string()],);

        // multiple matches via package name
        let restrict = DepRestrict::package("inherit");
        assert!(repo.iter_restrict(restrict).count() > 2);

        // all pkgs via repo ref
        let pkgs: Vec<_> = repo.iter_restrict(repo).try_collect().unwrap();
        assert_eq!(pkgs.len(), repo.len());
    }

    #[test]
    fn iter_ordered() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        let cpvs: Vec<_> = (0..100)
            .map(|x| Cpv::try_new(format!("cat/pkg-{x}")).unwrap())
            .collect();
        for cpv in &cpvs {
            temp.create_ebuild(cpv, &[]).unwrap();
        }
        config.finalize().unwrap();

        // valid pkgs
        let pkgs: Vec<_> = repo.iter_ordered().try_collect().unwrap();
        assert_ordered_eq!(pkgs.iter().map(|x| x.cpv()), &cpvs);

        // invalid pkg
        temp.create_ebuild("cat/pkg-100", &["EAPI=0"]).unwrap();
        let r: crate::Result<Vec<_>> = repo.iter_ordered().try_collect();
        assert_err_re!(r, "^invalid pkg: cat/pkg-100::test: unsupported EAPI: 0$");
    }

    #[test]
    fn iter_unordered() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
        let cpvs: Vec<_> = (0..100)
            .map(|x| Cpv::try_new(format!("cat/pkg-{x}")).unwrap())
            .collect();
        for cpv in &cpvs {
            temp.create_ebuild(cpv, &[]).unwrap();
        }
        config.finalize().unwrap();

        // valid pkgs
        let pkgs: Vec<_> = repo.iter_unordered().try_collect().unwrap();
        assert_unordered_eq!(pkgs.iter().map(|x| x.cpv()), &cpvs);

        // invalid pkg
        temp.create_ebuild("cat/pkg-100", &["EAPI=0"]).unwrap();
        let r: crate::Result<Vec<_>> = repo.iter_unordered().try_collect();
        assert_err_re!(r, "^invalid pkg: cat/pkg-100::test: unsupported EAPI: 0$");
    }

    #[test]
    fn get_pkg() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

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
        let data = test_data();
        let repo1 = data.ebuild_repo("primary").unwrap();
        assert_ordered_eq!(repo1.eclasses().iter().map(|e| e.name()), ["a", "c"]);
        let repo2 = data.ebuild_repo("secondary").unwrap();
        assert_ordered_eq!(repo2.eclasses().iter().map(|e| e.name()), ["a", "b", "c"]);
        // verify the overridden eclass is from the secondary repo
        let overridden_eclass = repo2.eclasses().get("c").unwrap();
        assert!(overridden_eclass.path().starts_with(repo2.path()));
    }

    #[test]
    fn arches() {
        let data = test_data();
        let repo = data.ebuild_repo("primary").unwrap();
        assert_ordered_eq!(repo.arches(), ["x86"]);
        let repo = data.ebuild_repo("secondary").unwrap();
        assert_ordered_eq!(repo.arches(), ["amd64", "x86"]);
    }

    #[test]
    fn licenses() {
        let data = test_data();
        let repo = data.ebuild_repo("primary").unwrap();
        assert_ordered_eq!(repo.licenses(), ["a"]);
        let repo = data.ebuild_repo("secondary").unwrap();
        assert_ordered_eq!(repo.licenses(), ["a", "b"]);
    }

    #[test]
    fn categories_xml() {
        let data = test_data();
        let repo = data.ebuild_repo("xml").unwrap();
        assert_eq!(repo.categories_xml().get("good").unwrap(), "good");
        // categories with invalid XML data don't have entries
        assert!(repo.categories_xml().get("bad").is_none());
        // categories without XML data don't have entries
        assert!(repo.categories_xml().get("pkg").is_none());
        // nonexistent categories don't have entries
        assert!(repo.categories_xml().get("nonexistent").is_none());
    }
}
