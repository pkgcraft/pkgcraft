use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, OnceLock};

use camino::Utf8PathBuf;
use itertools::Either;

use crate::dep::{Cpv, Dep};
use crate::dep::{DependencySet, Uri};
use crate::eapi::Eapi;
use crate::repo::ebuild::{Eclass, Repo};
use crate::repo::Repository;
use crate::shell::metadata::{Iuse, Key, Keyword, Metadata};
use crate::shell::phase::Phase;
use crate::traits::ToRef;
use crate::types::OrderedSet;
use crate::Error;

use super::{make_pkg_traits, Package, RepoPackage};

pub mod configured;
pub mod metadata;
use metadata::{Manifest, ManifestFile, XmlMetadata};
pub mod raw;
mod restrict;
pub use restrict::{MaintainerRestrict, Restrict};

pub trait EbuildPackage: Package {
    /// Return a package's set of effective USE choices.
    fn iuse_effective(&self) -> &OrderedSet<String>;
    /// Return a package's slot.
    fn slot(&self) -> &str;
}

impl<T> EbuildPackage for &T
where
    T: EbuildPackage,
{
    fn iuse_effective(&self) -> &OrderedSet<String> {
        (*self).iuse_effective()
    }
    fn slot(&self) -> &str {
        (*self).slot()
    }
}

#[derive(Debug)]
pub struct Pkg<'a> {
    cpv: Cpv<String>,
    eapi: &'static Eapi,
    repo: &'a Repo,
    meta: Metadata<'a>,
    iuse_effective: OnceLock<OrderedSet<String>>,
    xml: OnceLock<Arc<XmlMetadata>>,
    manifest: OnceLock<Arc<Manifest>>,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> TryFrom<raw::Pkg<'a>> for Pkg<'a> {
    type Error = Error;

    fn try_from(pkg: raw::Pkg) -> crate::Result<Pkg> {
        let meta = pkg.load_or_source()?;
        Ok(Pkg {
            cpv: pkg.cpv,
            eapi: pkg.eapi,
            repo: pkg.repo,
            meta,
            iuse_effective: OnceLock::new(),
            xml: OnceLock::new(),
            manifest: OnceLock::new(),
        })
    }
}

impl<'a> Pkg<'a> {
    /// Return the path of the package's ebuild file path relative to the repository root.
    pub fn relpath(&self) -> Utf8PathBuf {
        self.cpv.relpath()
    }

    /// Return the absolute path of the package's ebuild file.
    pub fn abspath(&self) -> Utf8PathBuf {
        self.repo.path().join(self.relpath())
    }

    /// Return a package's ebuild file content.
    pub fn ebuild(&self) -> crate::Result<String> {
        fs::read_to_string(self.abspath()).map_err(|e| Error::IO(e.to_string()))
    }

    /// Return a package's description.
    pub fn description(&self) -> &str {
        self.meta.description()
    }

    /// Return a package's subslot.
    pub fn subslot(&self) -> &str {
        self.meta.slot().subslot().unwrap_or_else(|| self.slot())
    }

    /// Return a package's dependencies for a given iterable of descriptors.
    pub fn dependencies(&self, keys: &[Key]) -> DependencySet<&str, &Dep<String>> {
        use Key::*;

        // default to all dependency types defined by the package EAPI if no keys are passed
        let keys = if keys.is_empty() {
            Either::Left(self.eapi().dep_keys())
        } else {
            Either::Right(keys)
        };

        keys.into_iter()
            .filter_map(|k| match k {
                BDEPEND => Some(self.bdepend()),
                DEPEND => Some(self.depend()),
                IDEPEND => Some(self.idepend()),
                PDEPEND => Some(self.pdepend()),
                RDEPEND => Some(self.rdepend()),
                // non-dependency keys are ignored
                _ => None,
            })
            .flatten()
            .map(|d| d.to_ref())
            .collect()
    }

    /// Return a package's BDEPEND.
    pub fn bdepend(&self) -> &DependencySet<String, Dep<String>> {
        self.meta.bdepend()
    }

    /// Return a package's DEPEND.
    pub fn depend(&self) -> &DependencySet<String, Dep<String>> {
        self.meta.depend()
    }

    /// Return a package's IDEPEND.
    pub fn idepend(&self) -> &DependencySet<String, Dep<String>> {
        self.meta.idepend()
    }

    /// Return a package's PDEPEND.
    pub fn pdepend(&self) -> &DependencySet<String, Dep<String>> {
        self.meta.pdepend()
    }

    /// Return a package's RDEPEND.
    pub fn rdepend(&self) -> &DependencySet<String, Dep<String>> {
        self.meta.rdepend()
    }

    /// Return a package's LICENSE.
    pub fn license(&self) -> &DependencySet<String, String> {
        self.meta.license()
    }

    /// Return a package's PROPERTIES.
    pub fn properties(&self) -> &DependencySet<String, String> {
        self.meta.properties()
    }

    /// Return a package's REQUIRED_USE.
    pub fn required_use(&self) -> &DependencySet<String, String> {
        self.meta.required_use()
    }

    /// Return a package's RESTRICT.
    pub fn restrict(&self) -> &DependencySet<String, String> {
        self.meta.restrict()
    }

    /// Return a package's SRC_URI.
    pub fn src_uri(&self) -> &DependencySet<String, Uri> {
        self.meta.src_uri()
    }

    /// Return a package's homepage.
    pub fn homepage(&self) -> &OrderedSet<String> {
        self.meta.homepage()
    }

    /// Return a package's defined phases
    pub fn defined_phases(&self) -> &OrderedSet<&Phase> {
        self.meta.defined_phases()
    }

    /// Return a package's keywords.
    pub fn keywords(&self) -> &OrderedSet<Keyword<String>> {
        self.meta.keywords()
    }

    /// Return a package's IUSE.
    pub fn iuse(&self) -> &OrderedSet<Iuse<String>> {
        self.meta.iuse()
    }

    /// Return the ordered set of directly inherited eclasses for a package.
    pub fn inherit(&self) -> &OrderedSet<&Eclass> {
        self.meta.inherit()
    }

    /// Return the ordered set of inherited eclasses for a package.
    pub fn inherited(&self) -> &OrderedSet<&Eclass> {
        self.meta.inherited()
    }

    /// Return the checksum for a package.
    pub fn chksum(&self) -> &str {
        self.meta.chksum()
    }

    /// Return a package's XML metadata.
    pub fn xml(&self) -> &XmlMetadata {
        self.xml
            .get_or_init(|| self.repo().pkg_xml(self.cpv()))
            .as_ref()
    }

    /// Return a package's manifest.
    pub fn manifest(&self) -> &Manifest {
        self.manifest
            .get_or_init(|| self.repo().pkg_manifest(self.cpv()))
            .as_ref()
    }

    /// Return a package's distfiles.
    pub fn distfiles(&self) -> Vec<&ManifestFile> {
        // pull filenames from flattened SRC_URI
        let files: HashSet<_> = self
            .src_uri()
            .iter_flatten()
            .map(|u| u.filename())
            .collect();

        // filter distfiles to be package version specific
        self.manifest()
            .distfiles()
            .iter()
            .filter(|d| files.contains(d.name()))
            .collect()
    }
}

impl<'a> Package for Pkg<'a> {
    fn eapi(&self) -> &'static Eapi {
        self.eapi
    }

    fn cpv(&self) -> &Cpv<String> {
        &self.cpv
    }
}

impl<'a> RepoPackage for Pkg<'a> {
    type Repo = &'a Repo;

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

impl<'a> EbuildPackage for Pkg<'a> {
    fn iuse_effective(&self) -> &OrderedSet<String> {
        self.iuse_effective.get_or_init(|| {
            self.meta
                .iuse()
                .iter()
                .map(|x| x.flag().to_string())
                .collect()
        })
    }

    fn slot(&self) -> &str {
        self.meta.slot().slot()
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::eapi::{EAPI8, EAPI_LATEST_OFFICIAL};
    use crate::macros::assert_err_re;
    use crate::pkg::ebuild::metadata::Checksum;
    use crate::repo::PkgRepository;
    use crate::test::{assert_ordered_eq, assert_unordered_eq, TEST_DATA};

    use super::*;

    #[test]
    fn eapi() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // unknown
        let r = t.create_raw_pkg("cat/pkg-1", &["EAPI=unknown"]);
        assert_err_re!(r, r"unsupported EAPI: unknown");

        // quoted and commented
        let data = indoc::formatdoc! {r#"
            EAPI="8" # comment
            DESCRIPTION="testing EAPI"
            SLOT=0
        "#};
        let pkg = t.create_pkg_from_str("cat/pkg-1", &data).unwrap();
        assert_eq!(pkg.eapi(), &*EAPI8);

        // invalid with unquoted self reference
        let data = indoc::indoc! {r#"
            EAPI=$EAPI
            DESCRIPTION="testing EAPI"
            SLOT=0
        "#};
        let r = t.create_raw_pkg_from_str("cat/pkg-1", data);
        assert_err_re!(r, r"invalid EAPI: \$EAPI");

        // unmatched quotes
        let data = indoc::indoc! {r#"
            EAPI='8"
            DESCRIPTION="testing EAPI"
            SLOT=0
        "#};
        let r = t.create_raw_pkg_from_str("cat/pkg-1", data);
        assert_err_re!(r, r#"invalid EAPI: '8""#);

        // unknown with leading whitespace, single quotes, and varying whitespace comment
        let data = indoc::indoc! {r#"
             EAPI='unknown' 	# comment
            DESCRIPTION="testing EAPI"
            SLOT=0
        "#};
        let r = t.create_raw_pkg_from_str("cat/pkg-1", data);
        assert_err_re!(r, r"unsupported EAPI: unknown");
    }

    #[test]
    fn pkg_methods() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // temp repo ebuild creation defaults to the latest EAPI
        let raw_pkg = t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        let relpath = raw_pkg.relpath();
        let pkg: Pkg = raw_pkg.try_into().unwrap();
        assert_eq!(pkg.relpath(), relpath);
        assert!(!pkg.ebuild().unwrap().is_empty());
    }

    #[test]
    fn package_trait() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();
        t.create_raw_pkg("cat/pkg-1", &[]).unwrap();
        t.create_raw_pkg("cat/pkg-2", &["EAPI=8"]).unwrap();

        let mut iter = t.repo().iter();
        let pkg1 = iter.next().unwrap();
        let pkg2 = iter.next().unwrap();

        // temp repo ebuild creation defaults to the latest EAPI
        assert_eq!(pkg1.eapi(), *EAPI_LATEST_OFFICIAL);
        assert_eq!(pkg1.eapi(), &*EAPI8);
        assert_eq!(pkg1.cpv(), &Cpv::try_new("cat/pkg-1").unwrap());
        assert_eq!(pkg2.cpv(), &Cpv::try_new("cat/pkg-2").unwrap());

        // repo attribute allows recursion
        assert_eq!(pkg1.repo(), pkg2.repo());
        let mut i = pkg1.repo().iter();
        assert_eq!(pkg1, i.next().unwrap());
        assert_eq!(pkg2, i.next().unwrap());
    }

    #[test]
    fn slot_and_subslot() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // default
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        assert_eq!(pkg.slot(), "0");
        assert_eq!(pkg.subslot(), "0");

        // custom lacking subslot
        let pkg = t.create_pkg("cat/pkg-2", &["SLOT=1"]).unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "1");

        // custom with subslot
        let pkg = t.create_pkg("cat/pkg-3", &["SLOT=1/2"]).unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "2");
    }

    #[test]
    fn dependencies() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // none
        let pkg = t.create_pkg("cat/pkg-1", &[]).unwrap();
        assert!(pkg.dependencies(&[]).is_empty());
        assert!(pkg.dependencies(&[Key::DEPEND]).is_empty());
        // non-dependency keys are ignored
        assert!(pkg.dependencies(&[Key::LICENSE]).is_empty());

        // empty
        let pkg = t.create_pkg("cat/pkg-1", &["DEPEND="]).unwrap();
        assert!(pkg.dependencies(&[]).is_empty());
        assert!(pkg.dependencies(&[Key::DEPEND]).is_empty());
        assert!(pkg.dependencies(&[Key::DEPEND, Key::RDEPEND]).is_empty());

        // single
        let pkg = t.create_pkg("cat/pkg-1", &["DEPEND=a/b"]).unwrap();
        assert_eq!(pkg.dependencies(&[]).to_string(), "a/b");
        assert_eq!(pkg.dependencies(&[Key::DEPEND]).to_string(), "a/b");
        assert_eq!(pkg.dependencies(&[Key::DEPEND, Key::RDEPEND]).to_string(), "a/b");

        // overlapping
        let pkg = t
            .create_pkg("cat/pkg-1", &["DEPEND=a/b", "RDEPEND=a/b"])
            .unwrap();
        assert_eq!(pkg.dependencies(&[]).to_string(), "a/b");
        assert_eq!(pkg.dependencies(&[Key::RDEPEND]).to_string(), "a/b");
        assert_eq!(pkg.dependencies(&[Key::DEPEND, Key::RDEPEND]).to_string(), "a/b");

        // multiple
        let pkg = t
            .create_pkg("cat/pkg-1", &["DEPEND=a/b", "RDEPEND=c/d"])
            .unwrap();
        assert_eq!(pkg.dependencies(&[]).to_string(), "a/b c/d");
        assert_eq!(pkg.dependencies(&[Key::RDEPEND]).to_string(), "c/d");
        assert_eq!(pkg.dependencies(&[Key::DEPEND, Key::RDEPEND]).to_string(), "a/b c/d");
        assert_eq!(pkg.dependencies(&[Key::RDEPEND, Key::DEPEND]).to_string(), "c/d a/b");
        // non-dependency keys are ignored
        assert!(pkg.dependencies(&[Key::LICENSE]).is_empty());
    }

    #[test]
    fn description() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        let pkg = t.create_pkg("cat/pkg-1", &["DESCRIPTION=desc"]).unwrap();
        assert_eq!(pkg.description(), "desc");
    }

    #[test]
    fn homepage() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // none
        let pkg = t.create_pkg("cat/pkg-1", &["HOMEPAGE="]).unwrap();
        assert!(pkg.homepage().is_empty());

        // single-line
        let pkg = t.create_pkg("cat/pkg-1", &["HOMEPAGE=home"]).unwrap();
        assert_ordered_eq(pkg.homepage(), ["home"]);

        // multi-line
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let pkg = t
            .create_pkg("cat/pkg-1", &[&format!("HOMEPAGE={val}")])
            .unwrap();
        assert_ordered_eq(pkg.homepage(), ["a", "b", "c"]);
    }

    #[test]
    fn defined_phases() {
        // none
        let pkg = TEST_DATA.ebuild_pkg("=phases/none-0::metadata").unwrap();
        assert!(pkg.defined_phases().is_empty());

        // ebuild-defined
        let pkg = TEST_DATA.ebuild_pkg("=phases/direct-0::metadata").unwrap();
        assert_unordered_eq(
            pkg.defined_phases().iter().map(|p| p.to_string()),
            ["src_compile", "src_install", "src_prepare"],
        );

        // eclass-defined
        let pkg = TEST_DATA
            .ebuild_pkg("=phases/indirect-0::metadata")
            .unwrap();
        assert_unordered_eq(
            pkg.defined_phases().iter().map(|p| p.to_string()),
            ["src_install", "src_prepare", "src_test"],
        );
    }

    #[test]
    fn keywords() {
        // none
        let pkg = TEST_DATA.ebuild_pkg("=keywords/none-0::metadata").unwrap();
        assert!(pkg.keywords().is_empty());

        // empty
        let pkg = TEST_DATA.ebuild_pkg("=keywords/empty-0::metadata").unwrap();
        assert!(pkg.keywords().is_empty());

        // single-line
        let pkg = TEST_DATA
            .ebuild_pkg("=keywords/single-0::metadata")
            .unwrap();
        assert_ordered_eq(pkg.keywords().iter().map(|x| x.to_string()), ["amd64", "~arm64"]);

        // multi-line
        let pkg = TEST_DATA.ebuild_pkg("=keywords/multi-0::metadata").unwrap();
        assert_ordered_eq(pkg.keywords().iter().map(|x| x.to_string()), ["~amd64", "arm64"]);
    }

    #[test]
    fn iuse() {
        // none
        let pkg = TEST_DATA.ebuild_pkg("=iuse/none-8::metadata").unwrap();
        assert!(pkg.iuse().is_empty());

        // empty
        let pkg = TEST_DATA.ebuild_pkg("=iuse/empty-8::metadata").unwrap();
        assert!(pkg.iuse().is_empty());

        // single-line
        let pkg = TEST_DATA.ebuild_pkg("=iuse/single-8::metadata").unwrap();
        assert_ordered_eq(pkg.iuse().iter().map(|x| x.to_string()), ["a", "+b", "-c"]);

        // multi-line
        let pkg = TEST_DATA.ebuild_pkg("=iuse/multi-8::metadata").unwrap();
        assert_ordered_eq(pkg.iuse().iter().map(|x| x.to_string()), ["a", "+b", "-c"]);

        // incremental inherit
        let pkg = TEST_DATA.ebuild_pkg("=iuse/inherit-8::metadata").unwrap();
        assert_ordered_eq(
            pkg.iuse().iter().map(|x| x.to_string()),
            ["global", "ebuild", "eclass", "a", "b"],
        );
    }

    #[test]
    fn license() {
        // none
        let pkg = TEST_DATA.ebuild_pkg("=license/none-8::metadata").unwrap();
        assert!(pkg.iuse().is_empty());

        // empty
        let pkg = TEST_DATA.ebuild_pkg("=license/empty-8::metadata").unwrap();
        assert!(pkg.iuse().is_empty());

        // single-line
        let pkg = TEST_DATA.ebuild_pkg("=license/single-8::metadata").unwrap();
        assert_eq!(pkg.license().to_string(), "l1 l2");

        // multi-line
        let pkg = TEST_DATA.ebuild_pkg("=license/multi-8::metadata").unwrap();
        assert_eq!(pkg.license().to_string(), "l1 u? ( l2 )");

        // inherited and overridden
        let pkg = TEST_DATA
            .ebuild_pkg("=license/inherit-8::metadata")
            .unwrap();
        assert_eq!(pkg.license().to_string(), "l1");

        // inherited and appended
        let pkg = TEST_DATA.ebuild_pkg("=license/append-8::metadata").unwrap();
        assert_eq!(pkg.license().to_string(), "l2 l1");
    }

    #[test]
    fn properties() {
        // none
        let pkg = TEST_DATA
            .ebuild_pkg("=properties/none-8::metadata")
            .unwrap();
        assert!(pkg.properties().is_empty());

        // empty
        let pkg = TEST_DATA
            .ebuild_pkg("=properties/empty-8::metadata")
            .unwrap();
        assert!(pkg.properties().is_empty());

        // single-line
        let pkg = TEST_DATA
            .ebuild_pkg("=properties/single-8::metadata")
            .unwrap();
        assert_eq!(pkg.properties().to_string(), "1 2");

        // multi-line
        let pkg = TEST_DATA
            .ebuild_pkg("=properties/multi-8::metadata")
            .unwrap();
        assert_eq!(pkg.properties().to_string(), "u? ( 1 2 )");

        // non-incremental inherit (EAPI 7)
        let pkg = TEST_DATA
            .ebuild_pkg("=properties/inherit-7::metadata")
            .unwrap();
        assert_eq!(pkg.properties().to_string(), "global ebuild");

        // incremental inherit (EAPI 8)
        let pkg = TEST_DATA
            .ebuild_pkg("=properties/inherit-8::metadata")
            .unwrap();
        assert_eq!(pkg.properties().to_string(), "global ebuild eclass a b");
    }

    #[test]
    fn restrict() {
        // none
        let pkg = TEST_DATA.ebuild_pkg("=restrict/none-8::metadata").unwrap();
        assert!(pkg.restrict().is_empty());

        // empty
        let pkg = TEST_DATA.ebuild_pkg("=restrict/empty-8::metadata").unwrap();
        assert!(pkg.restrict().is_empty());

        // single-line
        let pkg = TEST_DATA
            .ebuild_pkg("=restrict/single-8::metadata")
            .unwrap();
        assert_eq!(pkg.restrict().to_string(), "1 2");

        // multi-line
        let pkg = TEST_DATA.ebuild_pkg("=restrict/multi-8::metadata").unwrap();
        assert_eq!(pkg.restrict().to_string(), "u? ( 1 2 )");

        // non-incremental inherit (EAPI 7)
        let pkg = TEST_DATA
            .ebuild_pkg("=restrict/inherit-7::metadata")
            .unwrap();
        assert_eq!(pkg.restrict().to_string(), "global ebuild");

        // incremental inherit (EAPI 8)
        let pkg = TEST_DATA
            .ebuild_pkg("=restrict/inherit-8::metadata")
            .unwrap();
        assert_eq!(pkg.restrict().to_string(), "global ebuild eclass a b");
    }

    #[test]
    fn required_use() {
        // none
        let pkg = TEST_DATA
            .ebuild_pkg("=required_use/none-8::metadata")
            .unwrap();
        assert!(pkg.required_use().is_empty());

        // empty
        let pkg = TEST_DATA
            .ebuild_pkg("=required_use/empty-8::metadata")
            .unwrap();
        assert!(pkg.required_use().is_empty());

        // single-line
        let pkg = TEST_DATA
            .ebuild_pkg("=required_use/single-8::metadata")
            .unwrap();
        assert_eq!(pkg.required_use().to_string(), "u1 u2");

        // multi-line
        let pkg = TEST_DATA
            .ebuild_pkg("=required_use/multi-8::metadata")
            .unwrap();
        assert_eq!(pkg.required_use().to_string(), "^^ ( u1 u2 )");

        // incremental inherit
        let pkg = TEST_DATA
            .ebuild_pkg("=required_use/inherit-8::metadata")
            .unwrap();
        assert_eq!(pkg.required_use().to_string(), "global ebuild eclass a b");
    }

    #[test]
    fn inherits() {
        // none
        let pkg = TEST_DATA.ebuild_pkg("=inherit/none-0::metadata").unwrap();
        assert!(pkg.inherit().is_empty());
        assert!(pkg.inherited().is_empty());

        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        let a = repo.eclasses().get("a").unwrap();
        let b = repo.eclasses().get("b").unwrap();

        // direct inherit
        let pkg = TEST_DATA.ebuild_pkg("=inherit/direct-0::metadata").unwrap();
        assert_ordered_eq(pkg.inherit(), [&a]);
        assert_ordered_eq(pkg.inherited(), [&a]);

        // indirect inherit
        let pkg = TEST_DATA
            .ebuild_pkg("=inherit/indirect-0::metadata")
            .unwrap();
        assert_ordered_eq(pkg.inherit(), [&b]);
        assert_ordered_eq(pkg.inherited(), [&b, &a]);
    }

    #[test]
    fn maintainers() {
        // none
        let pkg = TEST_DATA.ebuild_pkg("=pkg/none-1::xml").unwrap();
        assert!(pkg.xml().maintainers().is_empty());

        // invalid
        let pkg = TEST_DATA.ebuild_pkg("=pkg/bad-1::xml").unwrap();
        assert!(pkg.xml().maintainers().is_empty());

        // single
        let pkg = TEST_DATA.ebuild_pkg("=pkg/single-1::xml").unwrap();
        let m = pkg.xml().maintainers();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].email(), "a.person@email.com");
        assert_eq!(m[0].name(), Some("A Person"));

        // multiple
        let pkg = TEST_DATA.ebuild_pkg("=pkg/multiple-1::xml").unwrap();
        let m = pkg.xml().maintainers();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].email(), "a.person@email.com");
        assert_eq!(m[0].name(), Some("A Person"));
        assert_eq!(m[1].email(), "b.person@email.com");
        assert_eq!(m[1].name(), Some("B Person"));
    }

    #[test]
    fn upstream() {
        // none
        let pkg = TEST_DATA.ebuild_pkg("=pkg/none-1::xml").unwrap();
        assert!(pkg.xml().upstream().is_none());

        // invalid
        let pkg = TEST_DATA.ebuild_pkg("=pkg/bad-1::xml").unwrap();
        assert!(pkg.xml().upstream().is_none());

        // single
        let pkg = TEST_DATA.ebuild_pkg("=pkg/single-1::xml").unwrap();
        let m = pkg.xml().upstream().unwrap().remote_ids();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].site(), "github");
        assert_eq!(m[0].name(), "pkgcraft/pkgcraft");

        // multiple
        let pkg = TEST_DATA.ebuild_pkg("=pkg/multiple-1::xml").unwrap();
        let m = pkg.xml().upstream().unwrap().remote_ids();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].site(), "github");
        assert_eq!(m[0].name(), "pkgcraft/pkgcraft");
        assert_eq!(m[1].site(), "pypi");
        assert_eq!(m[1].name(), "pkgcraft");
    }

    #[test]
    fn local_use() {
        // none
        let pkg = TEST_DATA.ebuild_pkg("=pkg/none-1::xml").unwrap();
        assert!(pkg.xml().local_use().is_empty());

        // invalid
        let pkg = TEST_DATA.ebuild_pkg("=pkg/bad-1::xml").unwrap();
        assert!(pkg.xml().local_use().is_empty());

        // single
        let pkg = TEST_DATA.ebuild_pkg("=pkg/single-1::xml").unwrap();
        assert_eq!(pkg.xml().local_use().len(), 1);
        assert_eq!(pkg.xml().local_use().get("flag").unwrap(), "flag desc");

        // multiple
        let pkg = TEST_DATA.ebuild_pkg("=pkg/multiple-1::xml").unwrap();
        assert_eq!(pkg.xml().local_use().len(), 2);
        assert_eq!(pkg.xml().local_use().get("flag1").unwrap(), "flag1 desc");
        assert_eq!(pkg.xml().local_use().get("flag2").unwrap(), "flag2 desc");
    }

    #[test]
    fn long_description() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // none
        let pkg = TEST_DATA.ebuild_pkg("=pkg/none-1::xml").unwrap();
        assert!(pkg.xml().long_description().is_none());

        // invalid
        let pkg = TEST_DATA.ebuild_pkg("=pkg/bad-1::xml").unwrap();
        assert!(pkg.xml().long_description().is_none());

        // empty
        let pkg1 = t.create_pkg("empty/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <pkgmetadata>
                <longdescription>
                </longdescription>
            </pkgmetadata>
        "#};
        fs::write(pkg1.abspath().parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg2 = t.create_pkg("empty/pkg-2", &[]).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(pkg.xml().long_description().unwrap(), "");
        }

        // single
        let pkg1 = t.create_pkg("cat1/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <longdescription>
                    A wrapped
                    sentence.
                    Another sentence.

                    New paragraph.
                </longdescription>
            </pkgmetadata>
        "#};
        fs::write(pkg1.abspath().parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg2 = t.create_pkg("cat1/pkg-2", &[]).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(
                pkg.xml().long_description().unwrap(),
                "A wrapped sentence. Another sentence. New paragraph."
            );
        }

        // multiple
        let pkg1 = t.create_pkg("cat2/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <longdescription lang="en">
                    A wrapped
                    sentence.
                    Another sentence.

                    New paragraph.
                </longdescription>
                <longdescription lang="zx">
                    zx
                </longdescription>
            </pkgmetadata>
        "#};
        fs::write(pkg1.abspath().parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg2 = t.create_pkg("cat2/pkg-2", &[]).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(
                pkg.xml().long_description().unwrap(),
                "A wrapped sentence. Another sentence. New paragraph."
            );
        }
    }

    #[test]
    fn distfiles() {
        let mut config = Config::default();
        let t = config.temp_repo("test", 0, None).unwrap();

        // none
        let pkg = t.create_pkg("nomanifest/pkg-1", &[]).unwrap();
        assert!(pkg.distfiles().is_empty());

        // single
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing distfiles"
            SLOT=0
            SRC_URI="https://url/to/a.tar.gz"
        "#};
        let pkg1 = t.create_pkg_from_str("cat1/pkg-1", data).unwrap();
        let manifest = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
        "#};
        fs::write(pkg1.abspath().parent().unwrap().join("Manifest"), manifest).unwrap();
        let pkg2 = t.create_pkg_from_str("cat1/pkg-2", data).unwrap();
        for pkg in [pkg1, pkg2] {
            let dist = pkg.distfiles();
            assert_eq!(dist.len(), 1);
            assert_eq!(dist[0].name(), "a.tar.gz");
            assert_eq!(dist[0].size(), 1);
            assert_eq!(dist[0].checksums()[0], Checksum::try_new("BLAKE2B", "a").unwrap());
            assert_eq!(dist[0].checksums()[1], Checksum::try_new("SHA512", "b").unwrap());
        }

        // multiple
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing distfiles"
            SLOT=0
            SRC_URI="https://url/to/a.tar.gz"
        "#};
        let pkg1 = t.create_pkg_from_str("cat2/pkg-1", data).unwrap();
        let manifest = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
            DIST b.tar.gz 2 BLAKE2B c SHA512 d
            DIST c.tar.gz 3 BLAKE2B c SHA512 d
        "#};
        fs::write(pkg1.abspath().parent().unwrap().join("Manifest"), manifest).unwrap();
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing distfiles"
            SLOT=0
            SRC_URI="https://url/to/b.tar.gz"
        "#};
        let pkg2 = t.create_pkg_from_str("cat2/pkg-2", data).unwrap();
        let dist = pkg1.distfiles();
        assert_eq!(dist.len(), 1);
        assert_eq!(dist[0].name(), "a.tar.gz");
        assert_eq!(dist[0].size(), 1);
        assert_eq!(dist[0].checksums()[0], Checksum::try_new("BLAKE2B", "a").unwrap());
        assert_eq!(dist[0].checksums()[1], Checksum::try_new("SHA512", "b").unwrap());
        let dist = pkg2.distfiles();
        assert_eq!(dist.len(), 1);
        assert_eq!(dist[0].name(), "b.tar.gz");
        assert_eq!(dist[0].size(), 2);
        assert_eq!(dist[0].checksums()[0], Checksum::try_new("BLAKE2B", "c").unwrap());
        assert_eq!(dist[0].checksums()[1], Checksum::try_new("SHA512", "d").unwrap());
    }
}
