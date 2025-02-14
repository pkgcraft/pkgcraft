use std::sync::{Arc, OnceLock};
use std::{fmt, fs};

use camino::Utf8PathBuf;
use indexmap::{IndexMap, IndexSet};
use tracing::warn;

use crate::dep::{Cpv, Dep};
use crate::dep::{DependencySet, Uri};
use crate::eapi::Eapi;
use crate::fetch::Fetchable;
use crate::macros::bool_not_equal;
use crate::repo::ebuild::{EbuildRepo, Eclass};
use crate::repo::Repository;
use crate::shell::phase::Phase;
use crate::traits::{Contains, Intersects, ToRef};
use crate::types::OrderedSet;
use crate::{bash, Error};

use super::{make_pkg_traits, Package, RepoPackage};

mod configured;
pub use configured::EbuildConfiguredPkg;
pub mod iuse;
pub mod keyword;
pub mod manifest;
use manifest::Manifest;
pub mod metadata;
use metadata::{Key, Metadata};
mod raw;
pub use raw::EbuildRawPkg;
mod restrict;
pub use restrict::{MaintainerRestrict, Restrict};
pub mod xml;

#[derive(Debug)]
struct InternalEbuildPkg {
    cpv: Cpv,
    repo: EbuildRepo,
    meta: Metadata,
    data: OnceLock<String>,
    tree: OnceLock<bash::Tree<'static>>,
    iuse_effective: OnceLock<OrderedSet<String>>,
    metadata: OnceLock<Arc<xml::Metadata>>,
    manifest: OnceLock<Arc<Manifest>>,
}

#[derive(Clone)]
pub struct EbuildPkg(Arc<InternalEbuildPkg>);

make_pkg_traits!(EbuildPkg);

impl fmt::Debug for EbuildPkg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "EbuildPkg {{ {self} }}")
    }
}

impl TryFrom<EbuildRawPkg> for EbuildPkg {
    type Error = Error;

    fn try_from(pkg: EbuildRawPkg) -> crate::Result<Self> {
        Ok(Self(Arc::new(InternalEbuildPkg {
            cpv: pkg.cpv().clone(),
            repo: pkg.repo(),
            meta: pkg.metadata(true)?,
            data: OnceLock::new(),
            tree: OnceLock::new(),
            iuse_effective: OnceLock::new(),
            metadata: OnceLock::new(),
            manifest: OnceLock::new(),
        })))
    }
}

impl EbuildPkg {
    /// Return the path of the package's ebuild file path relative to the repository root.
    pub fn relpath(&self) -> Utf8PathBuf {
        self.0.cpv.relpath()
    }

    /// Return the absolute path of the package's ebuild file.
    pub fn path(&self) -> Utf8PathBuf {
        self.0.repo.path().join(self.relpath())
    }

    /// Return a package's ebuild file content.
    pub fn data(&self) -> &str {
        self.0
            .data
            .get_or_init(|| fs::read_to_string(self.path()).unwrap_or_default())
    }

    /// Return the mapping of global environment variables exported by the package.
    pub fn env(&self) -> crate::Result<IndexMap<String, String>> {
        let repo = &self.0.repo;
        self.0.repo.pool().source_env(repo, &self.0.cpv)
    }

    /// Run the pkg_pretend phase for the package.
    pub fn pretend(&self) -> crate::Result<Option<String>> {
        let repo = &self.0.repo;
        self.0.repo.pool().pretend(repo, &self.0.cpv)
    }

    /// Return the bash parse tree for the ebuild.
    pub fn tree(&self) -> &bash::Tree<'static> {
        self.0.tree.get_or_init(|| {
            // HACK: figure out better method for self-referential lifetimes
            let data: &'static [u8] = unsafe { std::mem::transmute(self.data().as_bytes()) };
            bash::Tree::new(data)
        })
    }

    /// Return true if a package is globally deprecated in its repo, false otherwise.
    pub fn deprecated(&self) -> bool {
        self.0
            .repo
            .metadata()
            .pkg_deprecated()
            .iter()
            .any(|x| self.intersects(x))
    }

    /// Return true if a package is VCS-based, false otherwise.
    pub fn live(&self) -> bool {
        self.properties().contains("live")
    }

    /// Return true if a package is globally masked in its repo, false otherwise.
    pub fn masked(&self) -> bool {
        self.0
            .repo
            .metadata()
            .pkg_mask()
            .iter()
            .any(|x| self.intersects(x))
    }

    /// Return a package's description.
    pub fn description(&self) -> &str {
        &self.0.meta.description
    }

    /// Return a package's slot.
    pub fn slot(&self) -> &str {
        self.0.meta.slot.slot()
    }

    /// Return a package's subslot.
    pub fn subslot(&self) -> &str {
        self.0.meta.slot.subslot().unwrap_or_else(|| self.slot())
    }

    /// Return a package's dependencies for a given iterable of descriptors.
    pub fn dependencies<I>(&self, keys: I) -> DependencySet<&Dep>
    where
        I: IntoIterator<Item = Key>,
    {
        // collapse duplicate keys
        let mut keys: IndexSet<_> = keys.into_iter().collect();
        if keys.is_empty() {
            // default to all package dependencies
            keys = self.eapi().dep_keys().clone();
        }

        keys.into_iter()
            .filter_map(|k| match k {
                Key::BDEPEND => Some(self.bdepend()),
                Key::DEPEND => Some(self.depend()),
                Key::IDEPEND => Some(self.idepend()),
                Key::PDEPEND => Some(self.pdepend()),
                Key::RDEPEND => Some(self.rdepend()),
                // non-dependency keys are ignored
                _ => None,
            })
            .flatten()
            .map(|d| d.to_ref())
            .collect()
    }

    /// Return a package's BDEPEND.
    pub fn bdepend(&self) -> &DependencySet<Dep> {
        &self.0.meta.bdepend
    }

    /// Return a package's DEPEND.
    pub fn depend(&self) -> &DependencySet<Dep> {
        &self.0.meta.depend
    }

    /// Return a package's IDEPEND.
    pub fn idepend(&self) -> &DependencySet<Dep> {
        &self.0.meta.idepend
    }

    /// Return a package's PDEPEND.
    pub fn pdepend(&self) -> &DependencySet<Dep> {
        &self.0.meta.pdepend
    }

    /// Return a package's RDEPEND.
    pub fn rdepend(&self) -> &DependencySet<Dep> {
        &self.0.meta.rdepend
    }

    /// Return a package's LICENSE.
    pub fn license(&self) -> &DependencySet<String> {
        &self.0.meta.license
    }

    /// Return a package's PROPERTIES.
    pub fn properties(&self) -> &DependencySet<String> {
        &self.0.meta.properties
    }

    /// Return a package's REQUIRED_USE.
    pub fn required_use(&self) -> &DependencySet<String> {
        &self.0.meta.required_use
    }

    /// Return a package's RESTRICT.
    pub fn restrict(&self) -> &DependencySet<String> {
        &self.0.meta.restrict
    }

    /// Return a package's SRC_URI.
    pub fn src_uri(&self) -> &DependencySet<Uri> {
        &self.0.meta.src_uri
    }

    /// Return a package's homepage.
    pub fn homepage(&self) -> &OrderedSet<String> {
        &self.0.meta.homepage
    }

    /// Return a package's defined phases
    pub fn defined_phases(&self) -> &OrderedSet<Phase> {
        &self.0.meta.defined_phases
    }

    /// Return a package's keywords.
    pub fn keywords(&self) -> &OrderedSet<keyword::Keyword> {
        &self.0.meta.keywords
    }

    /// Return a package's IUSE.
    pub fn iuse(&self) -> &OrderedSet<iuse::Iuse> {
        &self.0.meta.iuse
    }

    /// Return a package's set of effective USE choices.
    pub fn iuse_effective(&self) -> &OrderedSet<String> {
        self.0.iuse_effective.get_or_init(|| {
            self.0
                .meta
                .iuse
                .iter()
                .map(|x| x.flag().to_string())
                .collect()
        })
    }

    /// Return the ordered set of directly inherited eclasses for a package.
    pub fn inherit(&self) -> &OrderedSet<Eclass> {
        &self.0.meta.inherit
    }

    /// Return the ordered set of inherited eclasses for a package.
    pub fn inherited(&self) -> &OrderedSet<Eclass> {
        &self.0.meta.inherited
    }

    /// Return a package's shared metadata.
    pub fn metadata(&self) -> &xml::Metadata {
        self.0
            .metadata
            .get_or_init(|| self.0.repo.metadata().pkg_metadata(self.cpn()))
    }

    /// Return a package's manifest.
    pub fn manifest(&self) -> &Manifest {
        self.0
            .manifest
            .get_or_init(|| self.0.repo.metadata().pkg_manifest(self.cpn()))
    }

    /// Generate fetchable URIs for a package's SRC_URI targets.
    pub fn fetchables(
        &self,
        override_restrict: bool,
        use_default_mirrors: bool,
    ) -> IterFetchable {
        IterFetchable {
            pkg: self,
            uris: self.src_uri().iter_flatten(),
            override_restrict,
            use_default_mirrors,
        }
    }

    /// Return a package's distfile names.
    pub fn distfiles(&self) -> impl Iterator<Item = &str> {
        // TODO: Use inspect() instead of filter() to panic on invalid filenames that
        // should be caught during parsing once it is reworked to support custom errors.
        self.src_uri()
            .iter_flatten()
            .map(|x| x.filename())
            .filter(|x| !x.is_empty())
    }
}

pub struct IterFetchable<'a> {
    pkg: &'a EbuildPkg,
    uris: crate::dep::IterFlatten<'a, Uri>,
    override_restrict: bool,
    use_default_mirrors: bool,
}

impl Iterator for IterFetchable<'_> {
    type Item = crate::Result<Fetchable>;

    fn next(&mut self) -> Option<Self::Item> {
        self.uris.find_map(|uri| {
            match Fetchable::from_uri(uri, self.pkg, self.use_default_mirrors) {
                Ok(f) => Some(Ok(f)),
                Err(Error::RestrictedFetchable(f)) => {
                    if self.override_restrict {
                        Some(Ok(*f))
                    } else {
                        warn!("skipping restricted fetchable: {f}");
                        None
                    }
                }
                Err(Error::RestrictedFile(f)) => {
                    warn!("skipping restricted file: {f}");
                    None
                }
                Err(e) => Some(Err(e.into_pkg_err(self.pkg))),
            }
        })
    }
}

impl Package for EbuildPkg {
    fn eapi(&self) -> &'static Eapi {
        self.0.meta.eapi
    }

    fn cpv(&self) -> &Cpv {
        &self.0.cpv
    }
}

impl RepoPackage for EbuildPkg {
    type Repo = EbuildRepo;

    fn repo(&self) -> Self::Repo {
        self.0.repo.clone()
    }
}

impl Intersects<Dep> for EbuildPkg {
    fn intersects(&self, dep: &Dep) -> bool {
        bool_not_equal!(self.cpn(), dep.cpn());

        if let Some(val) = dep.slot() {
            bool_not_equal!(self.slot(), val);
        }

        if let Some(val) = dep.subslot() {
            bool_not_equal!(self.subslot(), val);
        }

        // TODO: compare usedeps to iuse_effective

        if let Some(val) = dep.repo() {
            bool_not_equal!(self.0.repo.name(), val);
        }

        if let Some(val) = dep.version() {
            self.cpv().version().intersects(val)
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::eapi::{EAPI8, EAPI_LATEST_OFFICIAL};
    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::repo::PkgRepository;
    use crate::test::assert_err_re;
    use crate::test::{assert_ordered_eq, test_data};

    use super::*;

    #[test]
    fn display_and_debug() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let pkg = repo.iter().next().unwrap().unwrap();
        let s = pkg.to_string();
        assert!(format!("{pkg:?}").contains(&s));
    }

    #[test]
    fn eapi() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        // unknown
        temp.create_ebuild("cat/pkg-1", &["EAPI=unknown"]).unwrap();
        let r = repo.get_pkg_raw("cat/pkg-1");
        assert_err_re!(r, r"unsupported EAPI: unknown");

        // quoted and commented
        let data = indoc::formatdoc! {r#"
            EAPI="8" # comment
            DESCRIPTION="testing EAPI"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();
        assert_eq!(pkg.eapi(), &*EAPI8);

        // invalid with unquoted self reference
        let data = indoc::indoc! {r#"
            EAPI=$EAPI
            DESCRIPTION="testing EAPI"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let r = repo.get_pkg_raw("cat/pkg-1");
        assert_err_re!(r, r#"invalid EAPI: "\$EAPI""#);

        // unmatched quotes
        let data = indoc::indoc! {r#"
            EAPI='8"
            DESCRIPTION="testing EAPI"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let r = repo.get_pkg_raw("cat/pkg-1");
        assert_err_re!(r, r#"invalid EAPI: "'8"#);

        // unknown with leading whitespace, single quotes, and varying whitespace comment
        let data = indoc::indoc! {r#"
             EAPI='unknown' 	# comment
            DESCRIPTION="testing EAPI"
            SLOT=0
        "#};
        temp.create_ebuild_from_str("cat/pkg-1", data).unwrap();
        let r = repo.get_pkg_raw("cat/pkg-1");
        assert_err_re!(r, r"unsupported EAPI: unknown");
    }

    #[test]
    fn pkg_methods() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        // temp repo ebuild creation defaults to the latest EAPI
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        let raw_pkg = repo.get_pkg_raw("cat/pkg-1").unwrap();
        let relpath = raw_pkg.relpath();
        let pkg: EbuildPkg = raw_pkg.try_into().unwrap();
        assert_eq!(pkg.relpath(), relpath);
        assert!(!pkg.data().is_empty());
    }

    #[test]
    fn package_trait() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = repo.get_pkg("cat/pkg-1").unwrap();

        assert_eq!(pkg.eapi(), *EAPI_LATEST_OFFICIAL);
        assert_eq!(pkg.cpv(), &Cpv::try_new("cat/pkg-1").unwrap());
        assert_eq!(pkg.repo(), repo.clone());
        assert_eq!(pkg, pkg.repo().iter().next().unwrap().unwrap());
    }

    #[test]
    fn intersects_dep() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let pkg = repo.get_pkg("slot/subslot-8").unwrap();

        for (s, expected) in [
            ("slot/subslot", true),
            ("=slot/subslot-1", false),
            ("=slot/subslot-8", true),
            ("slot/subslot:0", false),
            ("slot/subslot:1", true),
            ("slot/subslot:0/1", false),
            ("slot/subslot:1/2", true),
            ("slot/subslot::test", false),
            ("slot/subslot::metadata", true),
        ] {
            let dep: Dep = s.parse().unwrap();
            assert_eq!(pkg.intersects(&dep), expected, "failed for {s}");
        }
    }

    #[test]
    fn slot_and_subslot() {
        let data = test_data();

        // without slot
        let repo = data.ebuild_repo("bad").unwrap();
        let r = repo.get_pkg("slot/none-8");
        assert_err_re!(r, "missing required value: SLOT$");

        // without subslot
        let repo = data.ebuild_repo("metadata").unwrap();
        let pkg = repo.get_pkg("slot/slot-8").unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "1");

        // with subslot
        let pkg = repo.get_pkg("slot/subslot-8").unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "2");
    }

    #[test]
    fn dependencies() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        for key in EAPI_LATEST_OFFICIAL.dep_keys() {
            assert!(pkg.dependencies([*key]).is_empty());
        }
        assert!(pkg.dependencies([]).is_empty());
        assert!(pkg.dependencies([Key::DEPEND, Key::RDEPEND]).is_empty());

        // empty
        let pkg = repo.get_pkg("optional/empty-8").unwrap();
        for key in EAPI_LATEST_OFFICIAL.dep_keys() {
            assert!(pkg.dependencies([*key]).is_empty());
        }
        assert!(pkg.dependencies([]).is_empty());
        assert!(pkg.dependencies([Key::DEPEND, Key::RDEPEND]).is_empty());

        // single-line
        let pkg = repo.get_pkg("dependencies/single-8").unwrap();
        for key in EAPI_LATEST_OFFICIAL.dep_keys() {
            assert_eq!(pkg.dependencies([*key]).to_string(), "a/pkg b/pkg");
        }
        assert_eq!(pkg.dependencies([]).to_string(), "a/pkg b/pkg");
        assert_eq!(pkg.dependencies([Key::DEPEND, Key::RDEPEND]).to_string(), "a/pkg b/pkg");

        // multi-line
        let pkg = repo.get_pkg("dependencies/multi-8").unwrap();
        for key in EAPI_LATEST_OFFICIAL.dep_keys() {
            assert_eq!(pkg.dependencies([*key]).to_string(), "a/pkg u? ( b/pkg )");
        }
        assert_eq!(pkg.dependencies([]).to_string(), "a/pkg u? ( b/pkg )");
        assert_eq!(
            pkg.dependencies([Key::DEPEND, Key::RDEPEND]).to_string(),
            "a/pkg u? ( b/pkg )"
        );

        // non-dependency keys are ignored
        assert!(pkg.dependencies([Key::LICENSE]).is_empty());
    }

    #[test]
    fn deprecated() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let pkg = repo.get_pkg("deprecated/deprecated-0").unwrap();
        assert!(pkg.deprecated());
        let pkg = repo.get_pkg("deprecated/deprecated-1").unwrap();
        assert!(!pkg.deprecated());
    }

    #[test]
    fn live() {
        let data = test_data();
        let repo = data.ebuild_repo("qa-primary").unwrap();
        let pkg = repo.get_pkg("Keywords/KeywordsLive-9999").unwrap();
        assert!(pkg.live());
        let pkg = repo.get_pkg("Keywords/KeywordsLive-0").unwrap();
        assert!(!pkg.live());
    }

    #[test]
    fn masked() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let pkg = repo.get_pkg("masked/masked-0").unwrap();
        assert!(pkg.masked());
        let pkg = repo.get_pkg("masked/masked-1").unwrap();
        assert!(!pkg.masked());
    }

    #[test]
    fn description() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert_eq!(pkg.description(), "ebuild with no optional metadata fields");

        // none
        let repo = data.ebuild_repo("bad").unwrap();
        let r = repo.get_pkg("description/none-8");
        assert_err_re!(r, "missing required value: DESCRIPTION$");
    }

    #[test]
    fn homepage() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert!(pkg.homepage().is_empty());

        // empty
        let pkg = repo.get_pkg("optional/empty-8").unwrap();
        assert!(pkg.homepage().is_empty());

        // single-line
        let pkg = repo.get_pkg("homepage/single-8").unwrap();
        assert_ordered_eq!(
            pkg.homepage(),
            ["https://github.com/pkgcraft/1", "https://github.com/pkgcraft/2"],
        );

        // multi-line
        let pkg = repo.get_pkg("homepage/multi-8").unwrap();
        assert_ordered_eq!(
            pkg.homepage(),
            ["https://github.com/pkgcraft/1", "https://github.com/pkgcraft/2"],
        );

        // inherited and overridden
        let pkg = repo.get_pkg("homepage/inherit-8").unwrap();
        assert_ordered_eq!(pkg.homepage(), ["https://github.com/pkgcraft/1"]);

        // inherited and appended
        let pkg = repo.get_pkg("homepage/append-8").unwrap();
        assert_ordered_eq!(
            pkg.homepage(),
            ["https://github.com/pkgcraft/a", "https://github.com/pkgcraft/1"],
        );
    }

    #[test]
    fn defined_phases() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert!(pkg.defined_phases().is_empty());

        // ebuild-defined
        let pkg = repo.get_pkg("phases/direct-8").unwrap();
        assert_ordered_eq!(
            pkg.defined_phases().iter().map(|p| p.to_string()),
            ["src_compile", "src_install", "src_prepare"],
        );

        // eclass-defined
        let pkg = repo.get_pkg("phases/indirect-8").unwrap();
        assert_ordered_eq!(
            pkg.defined_phases().iter().map(|p| p.to_string()),
            ["src_install", "src_prepare", "src_test"],
        );
    }

    #[test]
    fn keywords() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert!(pkg.keywords().is_empty());

        // empty
        let pkg = repo.get_pkg("optional/empty-8").unwrap();
        assert!(pkg.keywords().is_empty());

        // single-line
        let pkg = repo.get_pkg("keywords/single-8").unwrap();
        assert_ordered_eq!(pkg.keywords().iter().map(|x| x.to_string()), ["amd64", "~arm64"]);

        // multi-line
        let pkg = repo.get_pkg("keywords/multi-8").unwrap();
        assert_ordered_eq!(pkg.keywords().iter().map(|x| x.to_string()), ["~amd64", "arm64"]);
    }

    #[test]
    fn iuse() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert!(pkg.iuse().is_empty());

        // empty
        let pkg = repo.get_pkg("optional/empty-8").unwrap();
        assert!(pkg.iuse().is_empty());

        // single-line
        let pkg = repo.get_pkg("iuse/single-8").unwrap();
        assert_ordered_eq!(pkg.iuse().iter().map(|x| x.to_string()), ["a", "+b", "-c"]);

        // multi-line
        let pkg = repo.get_pkg("iuse/multi-8").unwrap();
        assert_ordered_eq!(pkg.iuse().iter().map(|x| x.to_string()), ["a", "+b", "-c"]);

        // incremental inherit
        let pkg = repo.get_pkg("iuse/inherit-8").unwrap();
        assert_ordered_eq!(
            pkg.iuse().iter().map(|x| x.to_string()),
            ["global", "ebuild", "eclass", "a", "b"],
        );
    }

    #[test]
    fn license() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert!(pkg.iuse().is_empty());

        // empty
        let pkg = repo.get_pkg("optional/empty-8").unwrap();
        assert!(pkg.iuse().is_empty());

        // single-line
        let pkg = repo.get_pkg("license/single-8").unwrap();
        assert_eq!(pkg.license().to_string(), "l1 l2");

        // multi-line
        let pkg = repo.get_pkg("license/multi-8").unwrap();
        assert_eq!(pkg.license().to_string(), "l1 u? ( l2 )");

        // inherited and overridden
        let pkg = repo.get_pkg("license/inherit-8").unwrap();
        assert_eq!(pkg.license().to_string(), "l1");

        // inherited and appended
        let pkg = repo.get_pkg("license/append-8").unwrap();
        assert_eq!(pkg.license().to_string(), "l2 l1");
    }

    #[test]
    fn properties() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert!(pkg.properties().is_empty());

        // empty
        let pkg = repo.get_pkg("optional/empty-8").unwrap();
        assert!(pkg.properties().is_empty());

        // single-line
        let pkg = repo.get_pkg("properties/single-8").unwrap();
        assert_eq!(pkg.properties().to_string(), "1 2");

        // multi-line
        let pkg = repo.get_pkg("properties/multi-8").unwrap();
        assert_eq!(pkg.properties().to_string(), "u? ( 1 2 )");

        // non-incremental inherit (EAPI 7)
        let pkg = repo.get_pkg("properties/inherit-7").unwrap();
        assert_eq!(pkg.properties().to_string(), "global ebuild");

        // incremental inherit (EAPI 8)
        let pkg = repo.get_pkg("properties/inherit-8").unwrap();
        assert_eq!(pkg.properties().to_string(), "global ebuild eclass a b");
    }

    #[test]
    fn restrict() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert!(pkg.restrict().is_empty());

        // empty
        let pkg = repo.get_pkg("optional/empty-8").unwrap();
        assert!(pkg.restrict().is_empty());

        // single-line
        let pkg = repo.get_pkg("restrict/single-8").unwrap();
        assert_eq!(pkg.restrict().to_string(), "1 2");

        // multi-line
        let pkg = repo.get_pkg("restrict/multi-8").unwrap();
        assert_eq!(pkg.restrict().to_string(), "u? ( 1 2 )");

        // non-incremental inherit (EAPI 7)
        let pkg = repo.get_pkg("restrict/inherit-7").unwrap();
        assert_eq!(pkg.restrict().to_string(), "global ebuild");

        // incremental inherit (EAPI 8)
        let pkg = repo.get_pkg("restrict/inherit-8").unwrap();
        assert_eq!(pkg.restrict().to_string(), "global ebuild eclass a b");
    }

    #[test]
    fn required_use() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert!(pkg.required_use().is_empty());

        // empty
        let pkg = repo.get_pkg("optional/empty-8").unwrap();
        assert!(pkg.required_use().is_empty());

        // single-line
        let pkg = repo.get_pkg("required_use/single-8").unwrap();
        assert_eq!(pkg.required_use().to_string(), "u1 u2");

        // multi-line
        let pkg = repo.get_pkg("required_use/multi-8").unwrap();
        assert_eq!(pkg.required_use().to_string(), "^^ ( u1 u2 )");

        // incremental inherit
        let pkg = repo.get_pkg("required_use/inherit-8").unwrap();
        assert_eq!(pkg.required_use().to_string(), "global ebuild eclass a b");
    }

    #[test]
    fn inherits() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();

        // none
        let pkg = repo.get_pkg("optional/none-8").unwrap();
        assert!(pkg.inherit().is_empty());
        assert!(pkg.inherited().is_empty());

        let a = repo.eclasses().get("a").unwrap();
        let b = repo.eclasses().get("b").unwrap();

        // direct inherit
        let pkg = repo.get_pkg("inherit/direct-8").unwrap();
        assert_ordered_eq!(pkg.inherit(), [a]);
        assert_ordered_eq!(pkg.inherited(), [a]);

        // indirect inherit
        let pkg = repo.get_pkg("inherit/indirect-8").unwrap();
        assert_ordered_eq!(pkg.inherit(), [b]);
        assert_ordered_eq!(pkg.inherited(), [b, a]);
    }

    #[test]
    fn pkg_metadata() {
        let data = test_data();
        let repo = data.ebuild_repo("xml").unwrap();

        // none
        let pkg = repo.get_pkg("pkg/none-8").unwrap();
        assert!(pkg.metadata().is_empty());

        // invalid
        let pkg = repo.get_pkg("pkg/bad-8").unwrap();
        assert!(pkg.metadata().is_empty());

        // single
        let pkg = repo.get_pkg("pkg/single-8").unwrap();
        assert!(!pkg.metadata().is_empty());

        // multiple
        let pkg = repo.get_pkg("pkg/multiple-8").unwrap();
        assert!(!pkg.metadata().is_empty());
    }

    #[test]
    fn distfiles() {
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        // none
        temp.create_ebuild("nomanifest/pkg-1", &[]).unwrap();
        let pkg = repo.get_pkg("nomanifest/pkg-1").unwrap();
        assert!(pkg.distfiles().next().is_none());

        // single
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing distfiles"
            SLOT=0
            SRC_URI="https://url/to/a.tar.gz"
        "#};
        temp.create_ebuild_from_str("cat1/pkg-1", data).unwrap();
        let pkg1 = repo.get_pkg("cat1/pkg-1").unwrap();
        let manifest = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
        "#};
        fs::write(pkg1.path().parent().unwrap().join("Manifest"), manifest).unwrap();
        temp.create_ebuild_from_str("cat1/pkg-2", data).unwrap();
        let pkg2 = repo.get_pkg("cat1/pkg-2").unwrap();
        for pkg in [pkg1, pkg2] {
            assert_ordered_eq!(pkg.distfiles(), ["a.tar.gz"]);
        }

        // multiple
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing distfiles"
            SLOT=0
            SRC_URI="https://url/to/a.tar.gz"
        "#};
        temp.create_ebuild_from_str("cat2/pkg-1", data).unwrap();
        let pkg1 = repo.get_pkg("cat2/pkg-1").unwrap();
        let manifest = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
            DIST b.tar.gz 2 BLAKE2B c SHA512 d
            DIST c.tar.gz 3 BLAKE2B c SHA512 d
        "#};
        fs::write(pkg1.path().parent().unwrap().join("Manifest"), manifest).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing distfiles"
            SLOT=0
            SRC_URI="https://url/to/b.tar.gz"
        "#};
        temp.create_ebuild_from_str("cat2/pkg-2", data).unwrap();
        let pkg2 = repo.get_pkg("cat2/pkg-2").unwrap();
        assert_ordered_eq!(pkg1.distfiles(), ["a.tar.gz"]);
        assert_ordered_eq!(pkg2.distfiles(), ["b.tar.gz"]);
    }
}
