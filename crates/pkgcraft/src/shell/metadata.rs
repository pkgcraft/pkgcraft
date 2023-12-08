use std::collections::HashMap;
use std::io::{self, Write};
use std::str::FromStr;
use std::{fmt, fs};

use itertools::Itertools;
use scallop::{functions, variables};
use strum::{AsRefStr, Display, EnumString};
use tracing::warn;

use crate::dep::{self, Cpv, Dep, DependencySet, Slot, Uri};
use crate::eapi::Eapi;
use crate::files::atomic_write_file;
use crate::pkg::{ebuild::raw::Pkg, Package, RepoPackage, Source};
use crate::repo::ebuild::Repo;
use crate::traits::IntoOwned;
use crate::types::OrderedSet;
use crate::Error;

use super::{get_build_mut, BuildData};

#[derive(
    AsRefStr, EnumString, Display, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
#[strum(serialize_all = "UPPERCASE")]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub enum Key {
    BDEPEND,
    DEFINED_PHASES,
    DEPEND,
    DESCRIPTION,
    EAPI,
    HOMEPAGE,
    IDEPEND,
    INHERIT,
    IUSE,
    KEYWORDS,
    LICENSE,
    PDEPEND,
    PROPERTIES,
    RDEPEND,
    REQUIRED_USE,
    RESTRICT,
    SLOT,
    SRC_URI,
    // match ordering of previous implementations (although the cache format is unordered)
    INHERITED,
    CHKSUM,
}

/// Package IUSE.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Iuse<S> {
    pub(crate) flag: S,
    pub(crate) default: Option<bool>,
}

impl IntoOwned for Iuse<&str> {
    type Owned = Iuse<String>;

    fn into_owned(self) -> Self::Owned {
        Iuse {
            flag: self.flag.to_string(),
            default: self.default,
        }
    }
}

impl<S: fmt::Display> fmt::Display for Iuse<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let flag = &self.flag;
        match &self.default {
            Some(true) => write!(f, "+{flag}"),
            Some(false) => write!(f, "-{flag}"),
            None => write!(f, "{flag}"),
        }
    }
}

impl Iuse<String> {
    fn new(s: &str) -> crate::Result<Self> {
        dep::parse::iuse(s).into_owned()
    }

    /// Return an IUSE flag stripping defaults.
    pub fn flag(&self) -> &str {
        &self.flag
    }
}

/// Package keyword type.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
pub enum KeywordStatus {
    Disabled, // -arch
    Unstable, // ~arch
    Stable,   // arch
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Keyword<S> {
    pub(crate) status: KeywordStatus,
    pub(crate) arch: S,
}

impl IntoOwned for Keyword<&str> {
    type Owned = Keyword<String>;

    fn into_owned(self) -> Self::Owned {
        Keyword {
            status: self.status,
            arch: self.arch.to_string(),
        }
    }
}

impl Keyword<String> {
    fn new(s: &str) -> crate::Result<Self> {
        dep::parse::keyword(s).into_owned()
    }

    /// Return the architecture for a keyword without its status.
    pub fn arch(&self) -> &str {
        &self.arch
    }

    /// Return the keyword status.
    pub fn status(&self) -> KeywordStatus {
        self.status
    }
}

impl FromStr for Keyword<String> {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        Keyword::new(s)
    }
}

impl<S: fmt::Display> fmt::Display for Keyword<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let arch = &self.arch;
        match &self.status {
            KeywordStatus::Stable => write!(f, "{arch}"),
            KeywordStatus::Unstable => write!(f, "~{arch}"),
            KeywordStatus::Disabled => write!(f, "-{arch}"),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Metadata {
    description: String,
    slot: Slot<String>,
    bdepend: DependencySet<String, Dep>,
    depend: DependencySet<String, Dep>,
    idepend: DependencySet<String, Dep>,
    pdepend: DependencySet<String, Dep>,
    rdepend: DependencySet<String, Dep>,
    license: DependencySet<String, String>,
    properties: DependencySet<String, String>,
    required_use: DependencySet<String, String>,
    restrict: DependencySet<String, String>,
    src_uri: DependencySet<String, Uri>,
    homepage: OrderedSet<String>,
    defined_phases: OrderedSet<String>,
    keywords: OrderedSet<Keyword<String>>,
    iuse: OrderedSet<Iuse<String>>,
    inherit: OrderedSet<String>,
    inherited: OrderedSet<String>,
    chksum: String,
}

macro_rules! required {
    ($eapi:expr, $key:expr) => {
        if $eapi.mandatory_keys().contains($key) {
            return Err(Error::InvalidValue(format!("missing required value: {}", $key)));
        }
    };
}

impl Metadata {
    /// Populate a metadata field value using the current build state.
    fn populate(
        &mut self,
        build: &mut BuildData,
        key: &Key,
        eapi: &'static Eapi,
    ) -> crate::Result<()> {
        use Key::*;
        match key {
            CHKSUM => required!(eapi, key),
            DEFINED_PHASES => {
                let phase_names: OrderedSet<_> = eapi
                    .phases()
                    .iter()
                    .filter_map(|p| functions::find(p).map(|_| p.short_name().to_string()))
                    .sorted()
                    .collect();
                if phase_names.is_empty() {
                    required!(eapi, key);
                } else {
                    self.defined_phases = phase_names;
                }
            }
            INHERIT => {
                let eclasses = &build.inherit;
                if eclasses.is_empty() {
                    required!(eapi, key);
                } else {
                    self.inherit = eclasses.iter().map(|x| x.to_string()).collect();
                }
            }
            INHERITED => {
                let eclasses = &build.inherited;
                if eclasses.is_empty() {
                    required!(eapi, key);
                } else {
                    self.inherited = eclasses.iter().map(|x| x.to_string()).collect();
                }
            }
            key => {
                // TODO: create dependency sets for incrementals directly from build state
                if let Some(val) = variables::optional(key) {
                    let s = val.split_whitespace().join(" ");
                    self.deserialize(eapi, key, &s)?;
                } else {
                    required!(eapi, key);
                }
            }
        }

        Ok(())
    }

    /// Deserialize a metadata string value to its field value.
    fn deserialize(&mut self, eapi: &'static Eapi, key: &Key, val: &str) -> crate::Result<()> {
        use Key::*;
        match key {
            CHKSUM => self.chksum = val.to_string(),
            DESCRIPTION => self.description = val.to_string(),
            SLOT => self.slot = Slot::new(val)?,
            BDEPEND => self.bdepend = dep::parse::package_dependency_set(val, eapi)?,
            DEPEND => self.depend = dep::parse::package_dependency_set(val, eapi)?,
            IDEPEND => self.idepend = dep::parse::package_dependency_set(val, eapi)?,
            PDEPEND => self.pdepend = dep::parse::package_dependency_set(val, eapi)?,
            RDEPEND => self.rdepend = dep::parse::package_dependency_set(val, eapi)?,
            LICENSE => self.license = dep::parse::license_dependency_set(val)?,
            PROPERTIES => self.properties = dep::parse::properties_dependency_set(val)?,
            REQUIRED_USE => self.required_use = dep::parse::required_use_dependency_set(val, eapi)?,
            RESTRICT => self.restrict = dep::parse::restrict_dependency_set(val)?,
            SRC_URI => self.src_uri = dep::parse::src_uri_dependency_set(val, eapi)?,
            HOMEPAGE => self.homepage = val.split_whitespace().map(String::from).collect(),
            DEFINED_PHASES => {
                self.defined_phases = val.split_whitespace().map(String::from).sorted().collect()
            }
            KEYWORDS => {
                self.keywords = val
                    .split_whitespace()
                    .map(Keyword::new)
                    .collect::<crate::Result<OrderedSet<_>>>()?
            }
            IUSE => {
                self.iuse = val
                    .split_whitespace()
                    .map(Iuse::new)
                    .collect::<crate::Result<OrderedSet<_>>>()?
            }
            INHERIT => self.inherit = val.split_whitespace().map(String::from).collect(),
            INHERITED => self.inherited = val.split_whitespace().map(String::from).collect(),
            EAPI => {
                let sourced: &Eapi = val.try_into()?;
                if sourced != eapi {
                    return Err(Error::InvalidValue(format!(
                        "mismatched sourced and parsed EAPIs: {sourced} != {eapi}"
                    )));
                }
            }
        }

        Ok(())
    }

    /// Serialize [`Metadata`] to the given package's metadata/md5-cache file in the related repo.
    pub(crate) fn serialize(pkg: &Pkg) -> crate::Result<()> {
        // convert raw pkg into metadata via sourcing
        let meta: Metadata = pkg.try_into()?;
        let eapi = pkg.eapi();

        // return the MD5 checksum for a known eclass
        let eclass_chksum = |name: &str| -> &str {
            pkg.repo()
                .eclasses()
                .get(name)
                .expect("missing eclass")
                .chksum()
        };

        // convert metadata fields to metadata lines
        use Key::*;
        let mut data = vec![];
        for key in eapi.metadata_keys() {
            match key {
                CHKSUM => writeln!(&mut data, "_md5_={}", pkg.chksum())?,
                DESCRIPTION => writeln!(&mut data, "{key}={}", meta.description)?,
                SLOT => writeln!(&mut data, "{key}={}", meta.slot)?,
                BDEPEND => {
                    if !meta.bdepend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.bdepend)?;
                    }
                }
                DEPEND => {
                    if !meta.depend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.depend)?;
                    }
                }
                IDEPEND => {
                    if !meta.idepend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.idepend)?;
                    }
                }
                PDEPEND => {
                    if !meta.pdepend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.pdepend)?;
                    }
                }
                RDEPEND => {
                    if !meta.rdepend.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.rdepend)?;
                    }
                }
                LICENSE => {
                    if !meta.license.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.license)?;
                    }
                }
                PROPERTIES => {
                    if !meta.properties.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.properties)?;
                    }
                }
                REQUIRED_USE => {
                    if !meta.required_use.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.required_use)?;
                    }
                }
                RESTRICT => {
                    if !meta.restrict.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.restrict)?;
                    }
                }
                SRC_URI => {
                    if !meta.src_uri.is_empty() {
                        writeln!(&mut data, "{key}={}", meta.src_uri)?;
                    }
                }
                HOMEPAGE => {
                    if !meta.homepage.is_empty() {
                        let val = meta.homepage.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                DEFINED_PHASES => {
                    // PMS specifies if no phase functions are defined, a single hyphen is used.
                    if meta.defined_phases.is_empty() {
                        writeln!(&mut data, "{key}=-")?;
                    } else {
                        let val = meta.defined_phases.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                KEYWORDS => {
                    if !meta.keywords().is_empty() {
                        let val = meta.keywords.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                IUSE => {
                    if !meta.iuse().is_empty() {
                        let val = meta.iuse.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                INHERIT => {
                    if !meta.inherit().is_empty() {
                        let val = meta.inherit.iter().join(" ");
                        writeln!(&mut data, "{key}={val}")?;
                    }
                }
                INHERITED => {
                    if !meta.inherited.is_empty() {
                        let val = meta
                            .inherited
                            .iter()
                            .flat_map(|s| [s.as_str(), eclass_chksum(s)])
                            .join("\t");
                        writeln!(&mut data, "_eclasses_={val}")?;
                    }
                }
                EAPI => writeln!(&mut data, "{key}={eapi}")?,
            }
        }

        // determine metadata entry directory
        let dir = pkg
            .repo()
            .metadata()
            .cache_path()
            .join(pkg.cpv().category());

        // create metadata entry directory
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .map_err(|e| Error::IO(format!("failed creating metadata dir: {dir}: {e}")))?;
        }

        // atomically create metadata file
        let pf = pkg.pf();
        let path = dir.join(format!(".{pf}"));
        let new_path = dir.join(pf);
        atomic_write_file(&path, data, &new_path)
    }

    /// Verify a metadata entry is valid.
    pub(crate) fn verify(cpv: &Cpv, repo: &Repo) -> bool {
        Pkg::new(cpv.clone(), repo)
            .map(|p| Self::load(&p, false).is_err())
            .unwrap_or_default()
    }

    /// Deserialize a metadata entry for a given package into [`Metadata`].
    pub(crate) fn load(pkg: &Pkg, deserialize: bool) -> crate::Result<Self> {
        let eapi = pkg.eapi();
        let repo = pkg.repo();

        let path = repo.metadata().cache_path().join(pkg.cpv().to_string());
        let data = fs::read_to_string(&path).map_err(|e| {
            if e.kind() != io::ErrorKind::NotFound {
                warn!("error loading ebuild metadata: {path:?}: {e}");
            }
            Error::IO(format!("failed loading ebuild metadata: {path:?}: {e}"))
        })?;

        let mut data: HashMap<_, _> = data
            .lines()
            .filter_map(|l| {
                l.split_once('=').map(|(s, v)| match (s, v) {
                    ("_eclasses_", v) => ("INHERITED", v),
                    ("_md5_", v) => ("CHKSUM", v),
                    // single hyphen means no phases are defined as per PMS
                    ("DEFINED_PHASES", "-") => ("DEFINED_PHASES", ""),
                    _ => (s, v),
                })
            })
            .filter_map(|(k, v)| k.parse().ok().map(|k| (k, v)))
            .filter(|(k, _)| eapi.metadata_keys().contains(k))
            .collect();

        let mut meta = Self::default();

        // verify ebuild hash
        if let Some(val) = data.remove(&Key::CHKSUM) {
            if val != pkg.chksum() {
                return Err(Error::InvalidValue("mismatched ebuild checksum".to_string()));
            }

            if deserialize {
                meta.chksum = val.to_string();
            }
        } else {
            return Err(Error::InvalidValue("missing ebuild checksum".to_string()));
        }

        // verify eclass hashes
        if let Some(val) = data.remove(&Key::INHERITED) {
            for (name, chksum) in val.split_whitespace().tuples() {
                if !repo
                    .eclasses()
                    .get(name)
                    .map_or(false, |e| e.chksum() == chksum)
                {
                    return Err(Error::InvalidValue("mismatched eclass checksum".to_string()));
                }

                if deserialize {
                    meta.inherited.insert(name.to_string());
                }
            }
        }

        // deserialize values into metadata fields
        if deserialize {
            for key in eapi.mandatory_keys() {
                if !data.contains_key(key) {
                    return Err(Error::InvalidValue(format!("missing required value: {key}")));
                }
            }
            for (key, val) in data {
                meta.deserialize(eapi, &key, val)?;
            }
        }

        Ok(meta)
    }

    pub(crate) fn description(&self) -> &str {
        &self.description
    }

    pub(crate) fn slot(&self) -> &Slot<String> {
        &self.slot
    }

    pub(crate) fn bdepend(&self) -> &DependencySet<String, Dep> {
        &self.bdepend
    }

    pub(crate) fn depend(&self) -> &DependencySet<String, Dep> {
        &self.depend
    }

    pub(crate) fn idepend(&self) -> &DependencySet<String, Dep> {
        &self.idepend
    }

    pub(crate) fn pdepend(&self) -> &DependencySet<String, Dep> {
        &self.pdepend
    }

    pub(crate) fn rdepend(&self) -> &DependencySet<String, Dep> {
        &self.rdepend
    }

    pub(crate) fn license(&self) -> &DependencySet<String, String> {
        &self.license
    }

    pub(crate) fn properties(&self) -> &DependencySet<String, String> {
        &self.properties
    }

    pub(crate) fn required_use(&self) -> &DependencySet<String, String> {
        &self.required_use
    }

    pub(crate) fn restrict(&self) -> &DependencySet<String, String> {
        &self.restrict
    }

    pub(crate) fn src_uri(&self) -> &DependencySet<String, Uri> {
        &self.src_uri
    }

    pub(crate) fn homepage(&self) -> &OrderedSet<String> {
        &self.homepage
    }

    pub(crate) fn defined_phases(&self) -> &OrderedSet<String> {
        &self.defined_phases
    }

    pub(crate) fn keywords(&self) -> &OrderedSet<Keyword<String>> {
        &self.keywords
    }

    pub(crate) fn iuse(&self) -> &OrderedSet<Iuse<String>> {
        &self.iuse
    }

    pub(crate) fn inherit(&self) -> &OrderedSet<String> {
        &self.inherit
    }

    pub(crate) fn inherited(&self) -> &OrderedSet<String> {
        &self.inherited
    }

    pub(crate) fn chksum(&self) -> &str {
        &self.chksum
    }
}

impl TryFrom<&Pkg<'_>> for Metadata {
    type Error = Error;

    fn try_from(pkg: &Pkg) -> crate::Result<Self> {
        // TODO: run sourcing via an external process pool returning the requested variables
        pkg.source()?;

        let eapi = pkg.eapi();
        let build = get_build_mut();
        let mut meta = Self::default();

        // pull metadata values from build state
        for key in eapi.metadata_keys() {
            meta.populate(build, key, eapi)?;
        }

        Ok(meta)
    }
}
