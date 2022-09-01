use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, prelude::*};
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;

use super::{make_pkg_traits, Package};
use crate::metadata::ebuild::{Distfile, Maintainer, Manifest, Upstream, XmlMetadata};
use crate::metadata::Metadata;
use crate::repo::ebuild::Repo;
use crate::{atom, eapi, Error};

mod restrict;
pub use restrict::Restrict;

static EAPI_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^EAPI=['\"]?(?P<EAPI>[^'\"]*)['\"]?[\t ]*(?:#.*)?").unwrap());

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    path: Utf8PathBuf,
    atom: atom::Atom,
    eapi: &'static eapi::Eapi,
    repo: &'a Repo,
    meta: Metadata,
    xml: OnceCell<Arc<XmlMetadata>>,
    manifest: OnceCell<Arc<Manifest>>,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(path: &Utf8Path, repo: &'a Repo) -> crate::Result<Self> {
        let eapi = Pkg::parse_eapi(path)?;
        let atom = repo.atom_from_path(path)?;
        // TODO: compare ebuild mtime vs cache mtime
        let meta = match Metadata::load(&atom, eapi, repo) {
            Some(data) => data,
            None => Metadata::source(path, eapi, repo)?,
        };
        Ok(Pkg {
            path: path.to_path_buf(),
            atom,
            eapi,
            repo,
            meta,
            xml: OnceCell::new(),
            manifest: OnceCell::new(),
        })
    }

    /// Get the parsed EAPI from a given ebuild file.
    fn parse_eapi(path: &Utf8Path) -> crate::Result<&'static eapi::Eapi> {
        let mut eapi = &*eapi::EAPI0;
        let f = fs::File::open(path).map_err(|e| Error::IO(e.to_string()))?;
        let reader = io::BufReader::new(f);
        for line in reader.lines() {
            let line = line.map_err(|e| Error::IO(e.to_string()))?;
            match line.chars().next() {
                None | Some('#') => continue,
                _ => {
                    if let Some(c) = EAPI_LINE_RE.captures(&line) {
                        eapi = eapi::get_eapi(c.name("EAPI").unwrap().as_str())?;
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

    /// Return a package's homepage.
    pub fn homepage(&self) -> &[String] {
        self.meta.homepage()
    }

    /// Return a package's defined phases
    pub fn defined_phases(&self) -> &HashSet<String> {
        self.meta.defined_phases()
    }

    /// Return a package's keywords.
    pub fn keywords(&self) -> &IndexSet<String> {
        self.meta.keywords()
    }

    /// Return a package's IUSE.
    pub fn iuse(&self) -> &IndexSet<String> {
        self.meta.iuse()
    }

    /// Return the ordered set of directly inherited eclasses for a package.
    pub fn inherit(&self) -> &IndexSet<String> {
        self.meta.inherit()
    }

    /// Return the ordered set of inherited eclasses for a package.
    pub fn inherited(&self) -> &IndexSet<String> {
        self.meta.inherited()
    }

    /// Return a package's XML metadata.
    fn xml(&self) -> &XmlMetadata {
        self.xml
            .get_or_init(|| self.repo.pkg_xml(&self.atom))
            .as_ref()
    }

    /// Return a package's maintainers.
    pub fn maintainers(&self) -> &[Maintainer] {
        self.xml().maintainers()
    }

    /// Return a package's upstreams.
    pub fn upstreams(&self) -> &[Upstream] {
        self.xml().upstreams()
    }

    /// Return a package's local USE flag mapping.
    pub fn local_use(&self) -> &HashMap<String, String> {
        self.xml().local_use()
    }

    /// Return a package's long description.
    pub fn long_description(&self) -> Option<&str> {
        self.xml().long_desc()
    }

    /// Return a package's manifest.
    fn manifest(&self) -> &Manifest {
        self.manifest
            .get_or_init(|| self.repo.pkg_manifest(&self.atom))
            .as_ref()
    }

    /// Return a package's distfiles.
    pub fn distfiles(&self) -> &[Distfile] {
        // TODO: parse SRC_URI to determine version specific distfiles
        self.manifest().distfiles()
    }
}

impl AsRef<Utf8Path> for Pkg<'_> {
    fn as_ref(&self) -> &Utf8Path {
        self.path()
    }
}

impl<'a> Package for Pkg<'a> {
    type Repo = &'a Repo;

    fn atom(&self) -> &atom::Atom {
        &self.atom
    }

    fn eapi(&self) -> &'static eapi::Eapi {
        self.eapi
    }

    fn repo(&self) -> Self::Repo {
        self.repo
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::metadata::Key;
    use crate::pkg::Env::*;
    use crate::test::eq_sorted;

    use super::*;

    #[test]
    fn test_invalid_eapi() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        let path = t
            .create_ebuild("cat/pkg-1", [(Key::Eapi, "$EAPI")])
            .unwrap();
        let r = Pkg::new(&path, &repo);
        assert_err_re!(r, r"^invalid EAPI: \$EAPI");
        let path = t
            .create_ebuild("cat/pkg-1", [(Key::Eapi, "unknown")])
            .unwrap();
        let r = Pkg::new(&path, &repo);
        assert_err_re!(r, r"^unknown EAPI: unknown");
    }

    #[test]
    fn test_as_ref_path() {
        fn assert_path<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(pkg: P, path: Q) {
            assert_eq!(pkg.as_ref(), path.as_ref());
        }

        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        let path = t.create_ebuild("cat/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_path(pkg, &path);
    }

    #[test]
    fn test_pkg_methods() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // temp repo ebuild creation defaults to the latest EAPI
        let path = t.create_ebuild("cat/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.path(), &path);
        assert!(!pkg.ebuild().unwrap().is_empty());
    }

    #[test]
    fn test_package_trait() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        t.create_ebuild("cat/pkg-1", []).unwrap();
        t.create_ebuild("cat/pkg-2", [(Key::Eapi, "0")]).unwrap();

        let mut iter = repo.iter();
        let pkg1 = iter.next().unwrap();
        let pkg2 = iter.next().unwrap();

        // temp repo ebuild creation defaults to the latest EAPI
        assert_eq!(pkg1.eapi(), &*eapi::EAPI_LATEST);
        assert_eq!(pkg2.eapi(), &*eapi::EAPI0);
        assert_eq!(pkg1.atom(), &atom::cpv("cat/pkg-1").unwrap());
        assert_eq!(pkg2.atom(), &atom::cpv("cat/pkg-2").unwrap());

        // repo attribute allows recursion
        assert_eq!(pkg1.repo(), pkg2.repo());
        let mut i = pkg1.repo().iter();
        assert_eq!(pkg1, i.next().unwrap());
        assert_eq!(pkg2, i.next().unwrap());
    }

    #[test]
    fn test_pkg_env() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // no revision
        let path = t.create_ebuild("cat/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.env(P), "pkg-1");
        assert_eq!(pkg.env(PN), "pkg");
        assert_eq!(pkg.env(PV), "1");
        assert_eq!(pkg.env(PR), "r0");
        assert_eq!(pkg.env(PVR), "1");
        assert_eq!(pkg.env(PF), "pkg-1");
        assert_eq!(pkg.env(CATEGORY), "cat");

        // revisioned
        let path = t.create_ebuild("cat/pkg-1-r2", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.env(P), "pkg-1");
        assert_eq!(pkg.env(PN), "pkg");
        assert_eq!(pkg.env(PV), "1");
        assert_eq!(pkg.env(PR), "r2");
        assert_eq!(pkg.env(PVR), "1-r2");
        assert_eq!(pkg.env(PF), "pkg-1-r2");
        assert_eq!(pkg.env(CATEGORY), "cat");

        // explicit r0 revision
        let path = t.create_ebuild("cat/pkg-2-r0", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.env(P), "pkg-2");
        assert_eq!(pkg.env(PN), "pkg");
        assert_eq!(pkg.env(PV), "2");
        assert_eq!(pkg.env(PR), "r0");
        assert_eq!(pkg.env(PVR), "2");
        assert_eq!(pkg.env(PF), "pkg-2");
        assert_eq!(pkg.env(CATEGORY), "cat");
    }

    #[test]
    fn test_slot() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // default (injected by create_ebuild())
        let path = t.create_ebuild("cat/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.slot(), "0");
        assert_eq!(pkg.subslot(), "0");

        // custom lacking subslot
        let path = t.create_ebuild("cat/pkg-2", [(Key::Slot, "1")]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "1");

        // custom with subslot
        let path = t.create_ebuild("cat/pkg-3", [(Key::Slot, "1/2")]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "2");
    }

    #[test]
    fn test_description() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        let path = t
            .create_ebuild("cat/pkg-1", [(Key::Description, "desc")])
            .unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.description(), "desc");
    }

    #[test]
    fn test_homepage() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // none
        let path = t
            .create_ebuild("cat/pkg-1", [(Key::Homepage, "-")])
            .unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.homepage().is_empty());

        // single line
        let path = t
            .create_ebuild("cat/pkg-1", [(Key::Homepage, "home")])
            .unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.homepage(), ["home"]);

        // multiple lines
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let path = t
            .create_ebuild("cat/pkg-1", [(Key::Homepage, val)])
            .unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.homepage(), ["a", "b", "c"]);
    }

    #[test]
    fn test_defined_phases() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // none
        let path = t.create_ebuild("cat/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.defined_phases().is_empty());

        // single
        let data = indoc::indoc! {r#"
            DESCRIPTION="testing defined phases"
            SLOT=0
            src_compile() { :; }
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.defined_phases(), &["compile"]));

        // multiple
        let data = indoc::indoc! {r#"
            EAPI=8
            DESCRIPTION="testing defined phases"
            SLOT=0
            src_prepare() { :; }
            src_compile() { :; }
            src_install() { :; }
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.defined_phases(), &["prepare", "compile", "install"]));

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
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.defined_phases(), &["prepare"]));

        // single overlapping from eclass and ebuild
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing defined phases"
            SLOT=0
            src_prepare() { :; }
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.defined_phases(), &["prepare"]));

        // multiple from eclasses
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1 e2
            DESCRIPTION="testing defined phases"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.defined_phases(), &["prepare", "compile", "install"]));

        // multiple from eclass and ebuild
        let data = indoc::indoc! {r#"
            EAPI=8
            inherit e1
            DESCRIPTION="testing defined phases"
            SLOT=0
            src_test() { :; }
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.defined_phases(), &["prepare", "test"]));
    }

    #[test]
    fn test_keywords() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // none
        let path = t.create_ebuild("cat/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.keywords().is_empty());

        // single line
        let path = t
            .create_ebuild("cat/pkg-1", [(Key::Keywords, "a b")])
            .unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.keywords(), &["a", "b"]));

        // multiple lines
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let path = t
            .create_ebuild("cat/pkg-1", [(Key::Keywords, val)])
            .unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.keywords(), &["a", "b", "c"]));
    }

    #[test]
    fn test_iuse() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // none
        let path = t.create_ebuild("cat/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.iuse().is_empty());

        // single line
        let path = t.create_ebuild("cat/pkg-1", [(Key::Iuse, "a b")]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.iuse(), &["a", "b"]));

        // multiple lines
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let path = t.create_ebuild("cat/pkg-1", [(Key::Iuse, val)]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.iuse(), &["a", "b", "c"]));

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
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.iuse(), &["use1"]));

        // inherited from multiple eclasses
        let data = indoc::indoc! {r#"
            inherit use1 use2
            DESCRIPTION="testing inherited IUSE"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.iuse(), &["use1", "use2"]));

        // accumulated from single eclass
        let data = indoc::indoc! {r#"
            inherit use1
            DESCRIPTION="testing accumulated IUSE"
            IUSE="a"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.iuse(), &["a", "use1"]));

        // accumulated from multiple eclasses
        let data = indoc::indoc! {r#"
            inherit use1 use2
            DESCRIPTION="testing accumulated IUSE"
            IUSE="a"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.iuse(), &["a", "use1", "use2"]));
    }

    #[test]
    fn test_inherits() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // none
        let path = t.create_ebuild("cat/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
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
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.inherit(), &["eclass1"]));
        assert!(eq_sorted(pkg.inherited(), &["eclass1"]));

        // eclass with indirect inherit
        let data = indoc::indoc! {r#"
            inherit eclass2
            DESCRIPTION="testing inherits"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.inherit(), &["eclass2"]));
        assert!(eq_sorted(pkg.inherited(), &["eclass2", "eclass1"]));

        // multiple inherits
        let data = indoc::indoc! {r#"
            inherit eclass1 eclass2
            DESCRIPTION="testing inherits"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.inherit(), &["eclass1", "eclass2"]));
        assert!(eq_sorted(pkg.inherited(), &["eclass1", "eclass2"]));
    }

    #[test]
    fn test_maintainers() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("xml", 0).unwrap();

        // none
        let path = t.create_ebuild("noxml/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.maintainers().is_empty());

        // single
        let path = t.create_ebuild("cat1/pkg-1", []).unwrap();
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
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat1/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let m = pkg.maintainers();
            assert_eq!(m.len(), 1);
            assert_eq!(m[0].email(), "a.person@email.com");
            assert_eq!(m[0].name(), Some("A Person"));
        }

        // multiple
        let path = t.create_ebuild("cat2/pkg-1", []).unwrap();
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
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat2/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let m = pkg.maintainers();
            assert_eq!(m.len(), 2);
            assert_eq!(m[0].email(), "a.person@email.com");
            assert_eq!(m[0].name(), Some("A Person"));
            assert_eq!(m[1].email(), "b.person@email.com");
            assert_eq!(m[1].name(), Some("B Person"));
        }
    }

    #[test]
    fn test_upstreams() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("xml", 0).unwrap();

        // none
        let path = t.create_ebuild("noxml/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.upstreams().is_empty());

        // single
        let path = t.create_ebuild("cat1/pkg-1", []).unwrap();
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
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat1/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let m = pkg.upstreams();
            assert_eq!(m.len(), 1);
            assert_eq!(m[0].site(), "github");
            assert_eq!(m[0].name(), "pkgcraft/pkgcraft");
        }

        // multiple
        let path = t.create_ebuild("cat2/pkg-1", []).unwrap();
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
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat2/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let m = pkg.upstreams();
            assert_eq!(m.len(), 2);
            assert_eq!(m[0].site(), "github");
            assert_eq!(m[0].name(), "pkgcraft/pkgcraft");
            assert_eq!(m[1].site(), "pypi");
            assert_eq!(m[1].name(), "pkgcraft");
        }
    }

    #[test]
    fn test_local_use() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("xml", 0).unwrap();

        // none
        let path = t.create_ebuild("noxml/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.local_use().is_empty());

        // single
        let path = t.create_ebuild("cat1/pkg-1", []).unwrap();
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
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat1/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(pkg.local_use().len(), 1);
            assert_eq!(pkg.local_use().get("flag").unwrap(), "flag desc");
        }

        // multiple
        let path = t.create_ebuild("cat2/pkg-1", []).unwrap();
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
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat2/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(pkg.local_use().len(), 2);
            assert_eq!(pkg.local_use().get("flag1").unwrap(), "flag1 desc");
            assert_eq!(pkg.local_use().get("flag2").unwrap(), "flag2 desc");
        }
    }

    #[test]
    fn test_long_description() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("xml", 0).unwrap();

        // none
        let path = t.create_ebuild("noxml/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.long_description().is_none());

        // empty
        let path = t.create_ebuild("empty/pkg-1", []).unwrap();
        let data = indoc::indoc! {r#"
            <pkgmetadata>
                <longdescription>
                </longdescription>
            </pkgmetadata>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("empty/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(pkg.long_description().unwrap(), "");
        }

        // invalid XML
        let path = t.create_ebuild("invalid/pkg-1", []).unwrap();
        let data = indoc::indoc! {r#"
            <pkgmetadata>
                <longdescription>
                    long description
                </longdescription>
            </pkg>
        "#};
        fs::write(path.parent().unwrap().join("metadata.xml"), data).unwrap();
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("invalid/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert!(pkg.long_description().is_none());
        }

        // single
        let path = t.create_ebuild("cat1/pkg-1", []).unwrap();
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
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat1/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(
                pkg.long_description().unwrap(),
                "A wrapped sentence. Another sentence.  New paragraph."
            );
        }

        // multiple
        let path = t.create_ebuild("cat2/pkg-1", []).unwrap();
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
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat2/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            assert_eq!(
                pkg.long_description().unwrap(),
                "A wrapped sentence. Another sentence.  New paragraph."
            );
        }
    }

    #[test]
    fn test_distfiles() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("manifest", 0).unwrap();

        // none
        let path = t.create_ebuild("nomanifest/pkg-1", []).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.distfiles().is_empty());

        // single
        let path = t.create_ebuild("cat1/pkg-1", []).unwrap();
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
        "#};
        fs::write(path.parent().unwrap().join("Manifest"), data).unwrap();
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat1/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let dist = pkg.distfiles();
            assert_eq!(dist.len(), 1);
            assert_eq!(dist[0].name(), "a.tar.gz");
            assert_eq!(dist[0].size(), 1);
            assert_eq!(dist[0].checksums()[0], ("blake2b".into(), "a".into()));
            assert_eq!(dist[0].checksums()[1], ("sha512".into(), "b".into()));
        }

        // multiple
        let path = t.create_ebuild("cat2/pkg-1", []).unwrap();
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
            DIST b.tar.gz 2 BLAKE2B c SHA512 d
        "#};
        fs::write(path.parent().unwrap().join("Manifest"), data).unwrap();
        let pkg1 = Pkg::new(&path, &repo).unwrap();
        let path = t.create_ebuild("cat2/pkg-2", []).unwrap();
        let pkg2 = Pkg::new(&path, &repo).unwrap();
        for pkg in [pkg1, pkg2] {
            let dist = pkg.distfiles();
            assert_eq!(dist.len(), 2);
            assert_eq!(dist[0].name(), "a.tar.gz");
            assert_eq!(dist[0].size(), 1);
            assert_eq!(dist[0].checksums()[0], ("blake2b".into(), "a".into()));
            assert_eq!(dist[0].checksums()[1], ("sha512".into(), "b".into()));
            assert_eq!(dist[1].name(), "b.tar.gz");
            assert_eq!(dist[1].size(), 2);
            assert_eq!(dist[1].checksums()[0], ("blake2b".into(), "c".into()));
            assert_eq!(dist[1].checksums()[1], ("sha512".into(), "d".into()));
        }
    }
}
