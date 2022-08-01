use std::collections::HashMap;
use std::io::{self, prelude::*};
use std::str::FromStr;
use std::sync::Arc;
use std::{fmt, fs, ptr};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use scallop::variables::string_value;
use tracing::warn;

use super::{make_pkg_traits, Package};
use crate::eapi::Key::*;
use crate::macros::build_from_paths;
use crate::metadata::ebuild::{Distfile, Maintainer, Manifest, Upstream, XmlMetadata};
use crate::pkgsh::source_ebuild;
use crate::repo::{ebuild::Repo, Repository};
use crate::{atom, eapi, pkg, restrict, Error};

static EAPI_LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^EAPI=['\"]?(?P<EAPI>[^'\"]*)['\"]?[\t ]*(?:#.*)?").unwrap());

#[derive(Debug, Default, Clone)]
struct Metadata<'a> {
    data: HashMap<eapi::Key, String>,
    description: OnceCell<&'a str>,
    slot: OnceCell<&'a str>,
    subslot: OnceCell<&'a str>,
    homepage: OnceCell<Vec<&'a str>>,
    keywords: OnceCell<IndexSet<&'a str>>,
    iuse: OnceCell<IndexSet<&'a str>>,
    inherit: OnceCell<IndexSet<&'a str>>,
    inherited: OnceCell<IndexSet<&'a str>>,
}

impl<'a> Metadata<'a> {
    /// Load metadata from cache.
    fn load(atom: &atom::Atom, eapi: &'static eapi::Eapi, repo: &Repo) -> Option<Self> {
        // TODO: validate cache entries in some fashion?
        let path = build_from_paths!(repo.path(), "metadata", "md5-cache", atom.to_string());
        match fs::read_to_string(&path) {
            Ok(s) => {
                let data = s
                    .lines()
                    .filter_map(|l| l.split_once('='))
                    .filter_map(|(k, v)| eapi::Key::from_str(k).ok().map(|k| (k, v)))
                    .filter(|(k, _)| eapi.metadata_keys().contains(k))
                    .map(|(k, v)| (k, v.to_string()))
                    .collect();
                Some(Self {
                    data,
                    ..Default::default()
                })
            }
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    warn!("error loading ebuild metadata: {:?}: {e}", &path);
                }
                None
            }
        }
    }

    /// Source ebuild to determine metadata.
    fn source(path: &Utf8Path, eapi: &'static eapi::Eapi) -> crate::Result<Self> {
        // TODO: run sourcing via an external process pool returning the requested variables
        source_ebuild(path)?;
        let mut data = HashMap::new();

        // verify sourced EAPI matches parsed EAPI
        let sourced_eapi = string_value("EAPI").unwrap_or_else(|| "0".into());
        if eapi::get_eapi(&sourced_eapi)? != eapi {
            return Err(Error::InvalidValue(format!(
                "mismatched sourced and parsed EAPIs: {sourced_eapi} != {eapi}"
            )));
        }

        // required metadata variables
        let mut missing = Vec::<&str>::new();
        for key in eapi.mandatory_keys() {
            match key.get(eapi) {
                Some(val) => drop(data.insert(*key, val)),
                None => missing.push(key.as_ref()),
            }
        }

        if !missing.is_empty() {
            missing.sort();
            let keys = missing.join(", ");
            return Err(Error::InvalidValue(format!("missing required values: {keys}")));
        }

        // metadata variables that default to empty
        for key in eapi.metadata_keys().difference(eapi.mandatory_keys()) {
            key.get(eapi).and_then(|v| data.insert(*key, v));
        }

        Ok(Self {
            data,
            ..Default::default()
        })
    }

    fn description(&'a self) -> &'a str {
        // mandatory key guaranteed to exist
        self.description
            .get_or_init(|| self.data.get(&Description).unwrap())
    }

    fn slot(&'a self) -> &'a str {
        self.slot.get_or_init(|| {
            // mandatory key guaranteed to exist
            let val = self.data.get(&Slot).unwrap();
            val.split_once('/').map_or(val, |x| x.0)
        })
    }

    fn subslot(&'a self) -> &'a str {
        self.subslot.get_or_init(|| {
            // mandatory key guaranteed to exist
            let val = self.data.get(&Slot).unwrap();
            val.split_once('/').map_or(val, |x| x.1)
        })
    }

    fn homepage(&'a self) -> &'a [&'a str] {
        self.homepage
            .get_or_init(|| {
                let val = self
                    .data
                    .get(&Homepage)
                    .map(|s| s.as_str())
                    .unwrap_or_default();
                val.split_whitespace().collect()
            })
            .as_slice()
    }

    fn keywords(&'a self) -> &'a IndexSet<&'a str> {
        self.keywords.get_or_init(|| {
            let val = self
                .data
                .get(&Keywords)
                .map(|s| s.as_str())
                .unwrap_or_default();
            val.split_whitespace().collect()
        })
    }

    fn iuse(&'a self) -> &'a IndexSet<&'a str> {
        self.iuse.get_or_init(|| {
            let val = self.data.get(&Iuse).map(|s| s.as_str()).unwrap_or_default();
            val.split_whitespace().collect()
        })
    }

    fn inherit(&'a self) -> &'a IndexSet<&'a str> {
        self.inherit.get_or_init(|| {
            let val = self
                .data
                .get(&Inherit)
                .map(|s| s.as_str())
                .unwrap_or_default();
            val.split_whitespace().collect()
        })
    }

    fn inherited(&'a self) -> &'a IndexSet<&'a str> {
        self.inherited.get_or_init(|| {
            let val = self
                .data
                .get(&Inherited)
                .map(|s| s.as_str())
                .unwrap_or_default();
            val.split_whitespace().collect()
        })
    }
}

#[derive(Debug, Clone)]
pub struct Pkg<'a> {
    path: Utf8PathBuf,
    atom: atom::Atom,
    eapi: &'static eapi::Eapi,
    repo: &'a Repo,
    data: Metadata<'a>,
    xml: OnceCell<Arc<XmlMetadata>>,
    manifest: OnceCell<Arc<Manifest>>,
}

make_pkg_traits!(Pkg<'_>);

impl<'a> Pkg<'a> {
    pub(crate) fn new(path: &Utf8Path, repo: &'a Repo) -> crate::Result<Self> {
        let eapi = Pkg::parse_eapi(path)?;
        let atom = repo.atom_from_path(path)?;
        // TODO: compare ebuild mtime vs cache mtime
        let data = match Metadata::load(&atom, eapi, repo) {
            Some(data) => data,
            None => Metadata::source(path, eapi)?,
        };
        Ok(Pkg {
            path: path.to_path_buf(),
            atom,
            eapi,
            repo,
            data,
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
    pub fn description(&'a self) -> &'a str {
        self.data.description()
    }

    /// Return a package's slot.
    pub fn slot(&'a self) -> &'a str {
        self.data.slot()
    }

    /// Return a package's subslot.
    pub fn subslot(&'a self) -> &'a str {
        self.data.subslot()
    }

    /// Return a package's homepage.
    pub fn homepage(&'a self) -> &'a [&'a str] {
        self.data.homepage()
    }

    /// Return a package's keywords.
    pub fn keywords(&'a self) -> &'a IndexSet<&'a str> {
        self.data.keywords()
    }

    /// Return a package's IUSE.
    pub fn iuse(&'a self) -> &'a IndexSet<&'a str> {
        self.data.iuse()
    }

    /// Return the ordered set of directly inherited eclasses for a package.
    pub fn inherit(&'a self) -> &'a IndexSet<&'a str> {
        self.data.inherit()
    }

    /// Return the ordered set of inherited eclasses for a package.
    pub fn inherited(&'a self) -> &'a IndexSet<&'a str> {
        self.data.inherited()
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

    /// Return a package's long description.
    pub fn distfiles(&self) -> &[Distfile] {
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

#[derive(Clone)]
pub enum Restrict {
    Custom(fn(&Pkg) -> bool),
    Description(restrict::Str),
}

impl fmt::Debug for Restrict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom(func) => write!(f, "Custom(func: {:?})", ptr::addr_of!(func)),
            Self::Description(r) => write!(f, "Description({r:?})"),
        }
    }
}

impl From<Restrict> for restrict::Restrict {
    fn from(r: Restrict) -> Self {
        Self::Pkg(pkg::Restrict::Ebuild(r))
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::macros::assert_err_re;
    use crate::pkg::Env::*;
    use crate::pkgsh::{BuildData, BUILD_DATA};
    use crate::test::eq_sorted;

    use super::*;

    #[test]
    fn test_invalid_eapi() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();
        let path = t.create_ebuild("cat/pkg-1", [(Eapi, "$EAPI")]).unwrap();
        let r = Pkg::new(&path, &repo);
        assert_err_re!(r, r"^invalid EAPI: \$EAPI");
        let path = t.create_ebuild("cat/pkg-1", [(Eapi, "unknown")]).unwrap();
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
        t.create_ebuild("cat/pkg-2", [(Eapi, "0")]).unwrap();

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
        let path = t.create_ebuild("cat/pkg-2", [(Slot, "1")]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "1");

        // custom with subslot
        let path = t.create_ebuild("cat/pkg-3", [(Slot, "1/2")]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.slot(), "1");
        assert_eq!(pkg.subslot(), "2");
    }

    #[test]
    fn test_description() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        let path = t
            .create_ebuild("cat/pkg-1", [(Description, "desc")])
            .unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.description(), "desc");
    }

    #[test]
    fn test_homepage() {
        let mut config = Config::new("pkgcraft", "", false).unwrap();
        let (t, repo) = config.temp_repo("test", 0).unwrap();

        // none
        let path = t.create_ebuild("cat/pkg-1", [(Homepage, "-")]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(pkg.homepage().is_empty());

        // single line
        let path = t.create_ebuild("cat/pkg-1", [(Homepage, "home")]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.homepage(), ["home"]);

        // multiple lines
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let path = t.create_ebuild("cat/pkg-1", [(Homepage, val)]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert_eq!(pkg.homepage(), ["a", "b", "c"]);
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
        let path = t.create_ebuild("cat/pkg-1", [(Keywords, "a b")]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.keywords(), &["a", "b"]));

        // multiple lines
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let path = t.create_ebuild("cat/pkg-1", [(Keywords, val)]).unwrap();
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
        let path = t.create_ebuild("cat/pkg-1", [(Iuse, "a b")]).unwrap();
        let pkg = Pkg::new(&path, &repo).unwrap();
        assert!(eq_sorted(pkg.iuse(), &["a", "b"]));

        // multiple lines
        let val = indoc::indoc! {"
            a
            b
            c
        "};
        let path = t.create_ebuild("cat/pkg-1", [(Iuse, val)]).unwrap();
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
        BuildData::reset();
        let data = indoc::indoc! {r#"
            inherit use1
            DESCRIPTION="testing inherited IUSE"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            let pkg = Pkg::new(&path, &repo).unwrap();
            assert!(eq_sorted(pkg.iuse(), &["use1"]));
        });

        // inherited from multiple eclasses
        BuildData::reset();
        let data = indoc::indoc! {r#"
            inherit use1 use2
            DESCRIPTION="testing inherited IUSE"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            let pkg = Pkg::new(&path, &repo).unwrap();
            assert!(eq_sorted(pkg.iuse(), &["use1", "use2"]));
        });

        // accumulated from single eclass
        BuildData::reset();
        let data = indoc::indoc! {r#"
            inherit use1
            DESCRIPTION="testing accumulated IUSE"
            IUSE="a"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            let pkg = Pkg::new(&path, &repo).unwrap();
            assert!(eq_sorted(pkg.iuse(), &["a", "use1"]));
        });

        // accumulated from multiple eclasses
        BuildData::reset();
        let data = indoc::indoc! {r#"
            inherit use1 use2
            DESCRIPTION="testing accumulated IUSE"
            IUSE="a"
            SLOT=0
        "#};
        let path = t.create_ebuild_raw("cat/pkg-1", data).unwrap();
        BUILD_DATA.with(|d| {
            d.borrow_mut().repo = repo.clone();
            let pkg = Pkg::new(&path, &repo).unwrap();
            assert!(eq_sorted(pkg.iuse(), &["a", "use1", "use2"]));
        });
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
