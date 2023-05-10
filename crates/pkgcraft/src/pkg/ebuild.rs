use std::collections::HashSet;
use std::fs;
use std::io::{self, prelude::*};
use std::str::FromStr;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use scallop::shell;

use crate::dep::{Cpv, Dep};
use crate::dep::{DepSet, Uri};
use crate::eapi::{self, Eapi, Feature, Operation};
use crate::pkgsh::{
    self,
    metadata::{Key, Metadata},
};
use crate::repo::{ebuild::Repo, Repository};
use crate::types::OrderedSet;
use crate::utils::relpath;
use crate::Error;

use super::{make_pkg_traits, BuildablePackage, Package};

pub mod metadata;
use metadata::{Manifest, ManifestFile, XmlMetadata};
mod restrict;
pub use restrict::{MaintainerRestrict, Restrict};

static EAPI_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^EAPI=['\"]?(?P<EAPI>[^'\"]*)['\"]?[\t ]*(?:#.*)?").unwrap());

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    path: Utf8PathBuf,
    cpv: Cpv,
    eapi: &'static Eapi,
    repo: &'a Repo,
    meta: Metadata,
    xml: OnceCell<Arc<XmlMetadata>>,
    manifest: OnceCell<Arc<Manifest>>,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(path: Utf8PathBuf, cpv: Cpv, repo: &'a Repo) -> crate::Result<Self> {
        let err = |e: Error| -> Error {
            Error::InvalidPkg {
                path: relpath(&path, repo.path()).expect("invalid relative pkg path"),
                err: e.to_string(),
            }
        };

        let eapi = Pkg::parse_eapi(&path).map_err(err)?;
        // TODO: compare ebuild mtime vs cache mtime
        let meta = match Metadata::load(&cpv, eapi, repo) {
            Some(data) => data,
            None => Metadata::source(&cpv, &path, eapi, repo).map_err(err)?,
        };
        Ok(Pkg {
            path,
            cpv,
            eapi,
            repo,
            meta,
            xml: OnceCell::new(),
            manifest: OnceCell::new(),
        })
    }

    /// Get the parsed EAPI from a given ebuild file.
    fn parse_eapi(path: &Utf8Path) -> crate::Result<&'static Eapi> {
        let mut eapi = &*eapi::EAPI0;
        let f = fs::File::open(path).map_err(|e| Error::IO(e.to_string()))?;
        let reader = io::BufReader::new(f);
        for line in reader.lines() {
            let line = line.map_err(|e| Error::IO(e.to_string()))?;
            match line.chars().next() {
                None | Some('#') => continue,
                _ => {
                    if let Some(c) = EAPI_LINE_RE.captures(&line) {
                        eapi = <&Eapi>::from_str(c.name("EAPI").unwrap().as_str())?;
                    }
                    break;
                }
            }
        }
        Ok(eapi)
    }

    /// Return a package's ebuild file path.
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Return a package's ebuild file content.
    pub fn ebuild(&self) -> crate::Result<String> {
        fs::read_to_string(&self.path).map_err(|e| Error::IO(e.to_string()))
    }

    /// Return a package's description.
    pub fn description(&self) -> &str {
        self.meta.description()
    }

    /// Return a package's slot.
    pub fn slot(&self) -> &str {
        self.meta.slot()
    }

    /// Return a package's subslot.
    pub fn subslot(&self) -> &str {
        self.meta.subslot().unwrap_or_else(|| self.slot())
    }

    /// Return a package's dependencies for a given iterable of descriptors.
    pub fn dependencies(&self, mut keys: &[Key]) -> DepSet<Dep> {
        use Key::*;

        // default to all dependency types in lexical order if no keys are passed
        if keys.is_empty() {
            keys = &[Bdepend, Depend, Idepend, Pdepend, Rdepend];
        }

        keys.iter()
            .filter_map(|k| match k {
                Bdepend => self.bdepend(),
                Depend => self.depend(),
                Idepend => self.idepend(),
                Pdepend => self.pdepend(),
                Rdepend => self.rdepend(),
                // non-dependency metadata keys are ignored
                _ => None,
            })
            .flatten()
            .cloned()
            .collect()
    }

    /// Return a package's BDEPEND.
    pub fn bdepend(&self) -> Option<&DepSet<Dep>> {
        self.meta.deps(Key::Bdepend)
    }

    /// Return a package's DEPEND.
    pub fn depend(&self) -> Option<&DepSet<Dep>> {
        self.meta.deps(Key::Depend)
    }

    /// Return a package's IDEPEND.
    pub fn idepend(&self) -> Option<&DepSet<Dep>> {
        self.meta.deps(Key::Idepend)
    }

    /// Return a package's PDEPEND.
    pub fn pdepend(&self) -> Option<&DepSet<Dep>> {
        self.meta.deps(Key::Pdepend)
    }

    /// Return a package's RDEPEND.
    pub fn rdepend(&self) -> Option<&DepSet<Dep>> {
        self.meta.deps(Key::Rdepend)
    }

    /// Return a package's LICENSE.
    pub fn license(&self) -> Option<&DepSet<String>> {
        self.meta.license()
    }

    /// Return a package's PROPERTIES.
    pub fn properties(&self) -> Option<&DepSet<String>> {
        self.meta.properties()
    }

    /// Return a package's REQUIRED_USE.
    pub fn required_use(&self) -> Option<&DepSet<String>> {
        self.meta.required_use()
    }

    /// Return a package's RESTRICT.
    pub fn restrict(&self) -> Option<&DepSet<String>> {
        self.meta.restrict()
    }

    /// Return a package's SRC_URI.
    pub fn src_uri(&self) -> Option<&DepSet<Uri>> {
        self.meta.src_uri()
    }

    /// Return a package's homepage.
    pub fn homepage(&self) -> &OrderedSet<String> {
        self.meta.homepage()
    }

    /// Return a package's defined phases
    pub fn defined_phases(&self) -> &OrderedSet<String> {
        self.meta.defined_phases()
    }

    /// Return a package's keywords.
    pub fn keywords(&self) -> &OrderedSet<String> {
        self.meta.keywords()
    }

    /// Return a package's IUSE.
    pub fn iuse(&self) -> &OrderedSet<String> {
        self.meta.iuse()
    }

    /// Return an unconfigured package's IUSE_EFFECTIVE.
    pub(crate) fn iuse_effective(&self) -> OrderedSet<&str> {
        if self.eapi().has(Feature::IuseDefaults) {
            self.iuse()
                .iter()
                .map(|s| s.trim_start_matches(['+', '-']))
                .collect()
        } else {
            self.iuse().iter().map(|s| s.as_str()).collect()
        }
    }

    /// Return the ordered set of directly inherited eclasses for a package.
    pub fn inherit(&self) -> &OrderedSet<String> {
        self.meta.inherit()
    }

    /// Return the ordered set of inherited eclasses for a package.
    pub fn inherited(&self) -> &OrderedSet<String> {
        self.meta.inherited()
    }

    /// Return a package's XML metadata.
    pub fn xml(&self) -> &XmlMetadata {
        self.xml
            .get_or_init(|| self.repo.pkg_xml(&self.cpv))
            .as_ref()
    }

    /// Return a package's manifest.
    pub fn manifest(&self) -> &Manifest {
        self.manifest
            .get_or_init(|| self.repo.pkg_manifest(&self.cpv))
            .as_ref()
    }

    /// Return a package's distfiles.
    pub fn distfiles(&self) -> Vec<&ManifestFile> {
        // pull filenames from flattened SRC_URI
        let files: HashSet<_> = self
            .src_uri()
            .map(|d| d.iter_flatten().map(|u| u.filename()).collect())
            .unwrap_or_default();

        // filter distfiles to be package version specific
        self.manifest()
            .distfiles()
            .iter()
            .filter(|d| files.contains(d.name()))
            .collect()
    }
}

impl AsRef<Utf8Path> for Pkg<'_> {
    fn as_ref(&self) -> &Utf8Path {
        self.path()
    }
}

impl<'a> Package for Pkg<'a> {
    type Repo = &'a Repo;

    fn cpv(&self) -> &Cpv {
        &self.cpv
    }

    fn eapi(&self) -> &'static Eapi {
        self.eapi
    }

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

impl<'a> BuildablePackage for Pkg<'a> {
    fn build(&self) -> crate::Result<()> {
        pkgsh::BuildData::from_pkg(self);
        pkgsh::source_ebuild(self.path())?;
        shell::toggle_restricted(false);
        for phase in self.eapi().operation(Operation::Build) {
            pkgsh::run_phase(*phase)?;
        }
        shell::toggle_restricted(true);
        Ok(())
    }

    fn pretend(&self) -> crate::Result<()> {
        pkgsh::BuildData::from_pkg(self);
        pkgsh::source_ebuild(self.path())?;
        shell::toggle_restricted(false);
        for phase in self.eapi().operation(Operation::Pretend) {
            pkgsh::run_phase(*phase)?;
        }
        shell::toggle_restricted(true);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::ebuild::metadata::Checksum;
    use crate::repo::PkgRepository;
    use crate::test::{assert_ordered_eq, assert_unordered_eq};

    use super::*;

    #[test]
    fn test_invalid_eapi() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["EAPI=$EAPI"]).unwrap();
        let r = Pkg::new(path, cpv, &repo);
        assert_err_re!(r, r"invalid EAPI: \$EAPI");
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["EAPI=unknown"]).unwrap();
        let r = Pkg::new(path, cpv, &repo);
        assert_err_re!(r, r"unknown EAPI: unknown");
    }

    #[test]
    fn test_variable_exports() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        // single
        let data = indoc::indoc! {r#"
            DESCRIPTION="testing defined phases"
            SLOT=0

            VARIABLE_GLOBAL="a"

            src_compile() {
                VARIABLE_GLOBAL="b"
                VARIABLE_DEFAULT="c"
                export VARIABLE_EXPORTED="d"
                local VARIABLE_LOCAL="e"
            }

            src_install() {
                [[ ${VARIABLE_GLOBAL} == "b" ]] \
                    || die "broken env saving for globals"

                [[ ${VARIABLE_DEFAULT} == "c" ]] \
                    || die "broken env saving for default"

                [[ ${VARIABLE_EXPORTED} == "d" ]] \
                    || die "broken env saving for exported"

                [[ $(printenv VARIABLE_EXPORTED ) == "d" ]] \
                    || die "broken env saving for exported"

                [[ -z ${VARIABLE_LOCAL} ]] \
                    || die "broken env saving for locals"
            }
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat1/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        pkg.build().unwrap();
    }

    #[test]
    fn test_as_ref_path() {
        fn assert_path<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(pkg: P, path: Q) {
            assert_eq!(pkg.as_ref(), path.as_ref());
        }

        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path.clone(), cpv, &repo).unwrap();
        assert_path(pkg, &path);
    }

    #[test]
    fn test_pkg_methods() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // temp repo ebuild creation defaults to the latest EAPI
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path.clone(), cpv, &repo).unwrap();
        assert_eq!(pkg.path(), &path);
        assert!(!pkg.ebuild().unwrap().is_empty());
    }

    #[test]
    fn test_package_trait() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();
        t.create_ebuild("cat/pkg-1", &[]).unwrap();
        t.create_ebuild("cat/pkg-2", &["EAPI=0"]).unwrap();

        let mut iter = repo.iter();
        let pkg1 = iter.next().unwrap();
        let pkg2 = iter.next().unwrap();

        // temp repo ebuild creation defaults to the latest EAPI
        assert_eq!(pkg1.eapi(), *eapi::EAPI_LATEST_OFFICIAL);
        assert_eq!(pkg2.eapi(), &*eapi::EAPI0);
        assert_eq!(pkg1.cpv(), &Cpv::new("cat/pkg-1").unwrap());
        assert_eq!(pkg2.cpv(), &Cpv::new("cat/pkg-2").unwrap());

        // repo attribute allows recursion
        assert_eq!(pkg1.repo(), pkg2.repo());
        let mut i = pkg1.repo().iter();
        assert_eq!(pkg1, i.next().unwrap());
        assert_eq!(pkg2, i.next().unwrap());
    }

    #[test]
    fn test_slot() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // default (injected by create_ebuild())
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_eq!(pkg.slot(), "0");
        assert_eq!(pkg.subslot(), "0");

        // custom lacking subslot
        let (path, cpv) = t.create_ebuild("cat/pkg-2", &["SLOT=1"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "1");

        // custom with subslot
        let (path, cpv) = t.create_ebuild("cat/pkg-3", &["SLOT=1/2"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "2");
    }

    #[test]
    fn test_description() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["DESCRIPTION=desc"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_eq!(pkg.description(), "desc");
    }

    #[test]
    fn test_homepage() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["HOMEPAGE="]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.homepage().is_empty());

        // single line
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["HOMEPAGE=home"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.homepage(), ["home"]);

        // multiple lines
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let (path, cpv) = t
            .create_ebuild("cat/pkg-1", &[&format!("HOMEPAGE={val}")])
            .unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.homepage(), ["a", "b", "c"]);
    }

    #[test]
    fn test_defined_phases() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.defined_phases().is_empty());

        // single
        let data = indoc::indoc! {r#"
            DESCRIPTION="testing defined phases"
            SLOT=0
            src_compile() { :; }
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_unordered_eq(pkg.defined_phases(), ["compile"]);

        // multiple
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing defined phases"
            SLOT=0
            src_prepare() { :; }
            src_compile() { :; }
            src_install() { :; }
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_unordered_eq(pkg.defined_phases(), ["prepare", "compile", "install"]);

        // create eclasses
        let eclass = indoc::indoc! {r#"
            EXPORT_FUNCTIONS src_prepare
            e1_src_prepare() { :; }
        "#};
        t.create_eclass("e1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            EXPORT_FUNCTIONS src_compile src_install
            e2_src_compile() { :; }
            e2_src_install() { :; }
        "#};
        t.create_eclass("e2", eclass).unwrap();

        // single from eclass
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing defined phases"
            SLOT=0
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_unordered_eq(pkg.defined_phases(), ["prepare"]);

        // single overlapping from eclass and ebuild
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing defined phases"
            SLOT=0
            src_prepare() { :; }
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_unordered_eq(pkg.defined_phases(), ["prepare"]);

        // multiple from eclasses
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1 e2
            DESCRIPTION="testing defined phases"
            SLOT=0
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_unordered_eq(pkg.defined_phases(), ["prepare", "compile", "install"]);

        // multiple from eclass and ebuild
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing defined phases"
            SLOT=0
            src_test() { :; }
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_unordered_eq(pkg.defined_phases(), ["prepare", "test"]);
    }

    #[test]
    fn test_keywords() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.keywords().is_empty());

        // single line
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["KEYWORDS=a b"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.keywords(), ["a", "b"]);

        // multiple lines
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let (path, cpv) = t
            .create_ebuild("cat/pkg-1", &[&format!("KEYWORDS={val}")])
            .unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.keywords(), ["a", "b", "c"]);
    }

    #[test]
    fn test_iuse() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.iuse().is_empty());

        // single line
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &["IUSE=a b"]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.iuse(), ["a", "b"]);

        // multiple lines
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let (path, cpv) = t
            .create_ebuild("cat/pkg-1", &[&format!("IUSE={val}")])
            .unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.iuse(), ["a", "b", "c"]);

        // create eclasses
        let eclass = indoc::indoc! {r#"
            IUSE="use1"
        "#};
        t.create_eclass("use1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            IUSE="use2"
        "#};
        t.create_eclass("use2", eclass).unwrap();

        // inherited from single eclass
        let data = indoc::indoc! {r#"
            inherit use1
            DESCRIPTION="testing inherited IUSE"
            SLOT=0
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.iuse(), ["use1"]);

        // inherited from multiple eclasses
        let data = indoc::indoc! {r#"
            inherit use1 use2
            DESCRIPTION="testing inherited IUSE"
            SLOT=0
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.iuse(), ["use1", "use2"]);

        // accumulated from single eclass
        let data = indoc::indoc! {r#"
            inherit use1
            DESCRIPTION="testing accumulated IUSE"
            IUSE="a"
            SLOT=0
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.iuse(), ["a", "use1"]);

        // accumulated from multiple eclasses
        let data = indoc::indoc! {r#"
            inherit use1 use2
            DESCRIPTION="testing accumulated IUSE"
            IUSE="a"
            SLOT=0
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.iuse(), ["a", "use1", "use2"]);
    }

    #[test]
    fn test_inherits() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("cat/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.inherit().is_empty());
        assert!(pkg.inherited().is_empty());

        // create eclasses
        let eclass = indoc::indoc! {r#"
            # eclass1
        "#};
        t.create_eclass("eclass1", eclass).unwrap();
        let eclass = indoc::indoc! {r#"
            # eclass2
            inherit eclass1
        "#};
        t.create_eclass("eclass2", eclass).unwrap();

        // single inherit
        let data = indoc::indoc! {r#"
            inherit eclass1
            DESCRIPTION="testing inherits"
            SLOT=0
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.inherit(), ["eclass1"]);
        assert_ordered_eq(pkg.inherited(), ["eclass1"]);

        // eclass with indirect inherit
        let data = indoc::indoc! {r#"
            inherit eclass2
            DESCRIPTION="testing inherits"
            SLOT=0
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.inherit(), ["eclass2"]);
        assert_ordered_eq(pkg.inherited(), ["eclass1", "eclass2"]);

        // multiple inherits
        let data = indoc::indoc! {r#"
            inherit eclass1 eclass2
            DESCRIPTION="testing inherits"
            SLOT=0
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert_ordered_eq(pkg.inherit(), ["eclass1", "eclass2"]);
        assert_ordered_eq(pkg.inherited(), ["eclass1", "eclass2"]);
    }

    #[test]
    fn test_maintainers() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("noxml/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.xml().maintainers().is_empty());

        // single
        let (path, cpv) = t.create_ebuild("cat1/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <maintainer type="person">
                    <email>a.person@email.com</email>
                    <name>A Person</name>
                </maintainer>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("cat1/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let m = pkg.xml().maintainers();
            assert_eq!(m.len(), 1);
            assert_eq!(m[0].email(), "a.person@email.com");
            assert_eq!(m[0].name(), Some("A Person"));
        }

        // multiple
        let (path, cpv) = t.create_ebuild("cat2/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <maintainer type="person">
                    <email>a.person@email.com</email>
                    <name>A Person</name>
                </maintainer>
                <maintainer type="person">
                    <email>b.person@email.com</email>
                    <name>B Person</name>
                </maintainer>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("cat2/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let m = pkg.xml().maintainers();
            assert_eq!(m.len(), 2);
            assert_eq!(m[0].email(), "a.person@email.com");
            assert_eq!(m[0].name(), Some("A Person"));
            assert_eq!(m[1].email(), "b.person@email.com");
            assert_eq!(m[1].name(), Some("B Person"));
        }
    }

    #[test]
    fn test_upstream() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("noxml/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.xml().upstream().is_none());

        // single
        let (path, cpv) = t.create_ebuild("cat1/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <upstream>
                    <remote-id type="github">pkgcraft/pkgcraft</remote-id>
                </upstream>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("cat1/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let m = pkg.xml().upstream().unwrap().remote_ids();
            assert_eq!(m.len(), 1);
            assert_eq!(m[0].site(), "github");
            assert_eq!(m[0].name(), "pkgcraft/pkgcraft");
        }

        // multiple
        let (path, cpv) = t.create_ebuild("cat2/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <upstream>
                    <remote-id type="github">pkgcraft/pkgcraft</remote-id>
                    <remote-id type="pypi">pkgcraft</remote-id>
                </upstream>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("cat2/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let m = pkg.xml().upstream().unwrap().remote_ids();
            assert_eq!(m.len(), 2);
            assert_eq!(m[0].site(), "github");
            assert_eq!(m[0].name(), "pkgcraft/pkgcraft");
            assert_eq!(m[1].site(), "pypi");
            assert_eq!(m[1].name(), "pkgcraft");
        }
    }

    #[test]
    fn test_local_use() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("noxml/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.xml().local_use().is_empty());

        // single
        let (path, cpv) = t.create_ebuild("cat1/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <use>
                    <flag name="flag">flag desc</flag>
                </use>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("cat1/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(pkg.xml().local_use().len(), 1);
            assert_eq!(pkg.xml().local_use().get("flag").unwrap(), "flag desc");
        }

        // multiple
        let (path, cpv) = t.create_ebuild("cat2/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">
            <pkgmetadata>
                <use lang="en">
                    <flag name="flag1">flag1 desc</flag>
                    <flag name="flag2">flag2 desc</flag>
                </use>
                <use lang="zx">
                    <flag name="flag1">flag1 desc</flag>
                    <flag name="flag2">flag2 desc</flag>
                </use>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("cat2/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(pkg.xml().local_use().len(), 2);
            assert_eq!(pkg.xml().local_use().get("flag1").unwrap(), "flag1 desc");
            assert_eq!(pkg.xml().local_use().get("flag2").unwrap(), "flag2 desc");
        }
    }

    #[test]
    fn test_long_description() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("noxml/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.xml().long_description().is_none());

        // empty
        let (path, cpv) = t.create_ebuild("empty/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <pkgmetadata>
                <longdescription>
                </longdescription>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("empty/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(pkg.xml().long_description().unwrap(), "");
        }

        // invalid XML
        let (path, cpv) = t.create_ebuild("invalid/pkg-1", &[]).unwrap();
        let data = indoc::indoc! {r#"
            <pkgmetadata>
                <longdescription>
                    long description
                </longdescription>
            </pkg>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("invalid/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert!(pkg.xml().long_description().is_none());
        }

        // single
        let (path, cpv) = t.create_ebuild("cat1/pkg-1", &[]).unwrap();
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
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("cat1/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(
                pkg.xml().long_description().unwrap(),
                "A wrapped sentence. Another sentence. New paragraph."
            );
        }

        // multiple
        let (path, cpv) = t.create_ebuild("cat2/pkg-1", &[]).unwrap();
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
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild("cat2/pkg-2", &[]).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(
                pkg.xml().long_description().unwrap(),
                "A wrapped sentence. Another sentence. New paragraph."
            );
        }
    }

    #[test]
    fn test_distfiles() {
        let mut config = Config::default();
        let (t, repo) = config.temp_repo("test", 0, None).unwrap();

        // none
        let (path, cpv) = t.create_ebuild("nomanifest/pkg-1", &[]).unwrap();
        let pkg = Pkg::new(path, cpv, &repo).unwrap();
        assert!(pkg.distfiles().is_empty());

        // single
        let data = indoc::indoc! {r#"
            DESCRIPTION="testing distfiles"
            SLOT=0
            SRC_URI="https://url/to/a.tar.gz"
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat1/pkg-1", data).unwrap();
        let manifest = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
        "#};
        fs::write(path.parent().unwrap().join("Manifest"), manifest).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let (path, cpv) = t.create_ebuild_raw("cat1/pkg-2", data).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let dist = pkg.distfiles();
            assert_eq!(dist.len(), 1);
            assert_eq!(dist[0].name(), "a.tar.gz");
            assert_eq!(dist[0].size(), 1);
            assert_eq!(dist[0].checksums()[0], Checksum::new("BLAKE2B", "a").unwrap());
            assert_eq!(dist[0].checksums()[1], Checksum::new("SHA512", "b").unwrap());
        }

        // multiple
        let data = indoc::indoc! {r#"
            DESCRIPTION="testing distfiles"
            SLOT=0
            SRC_URI="https://url/to/a.tar.gz"
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat2/pkg-1", data).unwrap();
        let manifest = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
            DIST b.tar.gz 2 BLAKE2B c SHA512 d
            DIST c.tar.gz 3 BLAKE2B c SHA512 d
        "#};
        fs::write(path.parent().unwrap().join("Manifest"), manifest).unwrap();
        let pkg1 = Pkg::new(path, cpv, &repo).unwrap();
        let data = indoc::indoc! {r#"
            DESCRIPTION="testing distfiles"
            SLOT=0
            SRC_URI="https://url/to/b.tar.gz"
        "#};
        let (path, cpv) = t.create_ebuild_raw("cat2/pkg-2", data).unwrap();
        let pkg2 = Pkg::new(path, cpv, &repo).unwrap();
        let dist = pkg1.distfiles();
        assert_eq!(dist.len(), 1);
        assert_eq!(dist[0].name(), "a.tar.gz");
        assert_eq!(dist[0].size(), 1);
        assert_eq!(dist[0].checksums()[0], Checksum::new("BLAKE2B", "a").unwrap());
        assert_eq!(dist[0].checksums()[1], Checksum::new("SHA512", "b").unwrap());
        let dist = pkg2.distfiles();
        assert_eq!(dist.len(), 1);
        assert_eq!(dist[0].name(), "b.tar.gz");
        assert_eq!(dist[0].size(), 2);
        assert_eq!(dist[0].checksums()[0], Checksum::new("BLAKE2B", "c").unwrap());
        assert_eq!(dist[0].checksums()[1], Checksum::new("SHA512", "d").unwrap());
    }
}
