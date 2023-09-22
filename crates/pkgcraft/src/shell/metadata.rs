use std::cmp::Ordering;
use std::collections::HashMap;
use std::str::FromStr;
use std::{fs, io};

use itertools::Itertools;
use scallop::{functions, variables};
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};
use tracing::warn;

use crate::dep::{self, Cpv, Dep, DepSet, Uri};
use crate::eapi::Eapi;
use crate::pkg::SourceablePackage;
use crate::pkg::{ebuild::RawPkg, Package};
use crate::repo::ebuild::Repo;
use crate::types::OrderedSet;
use crate::Error;

use super::{get_build_mut, BuildData};

#[derive(AsRefStr, EnumIter, EnumString, Display, Debug, PartialEq, Eq, Hash, Copy, Clone)]
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
    // last to match serialized data
    INHERITED,
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Key {
    pub(crate) fn get(&self, build: &mut BuildData, eapi: &'static Eapi) -> Option<String> {
        match self {
            Key::DEFINED_PHASES => {
                let mut phase_names: Vec<_> = eapi
                    .phases()
                    .iter()
                    .filter_map(|p| functions::find(p).map(|_| p.short_name()))
                    .collect();
                if phase_names.is_empty() {
                    None
                } else {
                    phase_names.sort_unstable();
                    Some(phase_names.join(" "))
                }
            }
            Key::INHERIT => {
                let inherit = &build.inherit;
                if inherit.is_empty() {
                    None
                } else {
                    Some(inherit.iter().join(" "))
                }
            }
            key => variables::optional(key).map(|s| s.split_whitespace().join(" ")),
        }
    }

    /// Convert a given key and value into a metadata entry line.
    fn line<S: std::fmt::Display>(&self, value: S) -> String {
        let var = match self {
            Key::INHERITED => "_eclasses_",
            key => key.as_ref(),
        };

        format!("{var}={value}")
    }
}

#[derive(Debug, Default)]
pub(crate) struct Metadata {
    description: String,
    slot: String,
    deps: HashMap<Key, DepSet<Dep>>,
    license: Option<DepSet<String>>,
    properties: Option<DepSet<String>>,
    required_use: Option<DepSet<String>>,
    restrict: Option<DepSet<String>>,
    src_uri: Option<DepSet<Uri>>,
    homepage: OrderedSet<String>,
    defined_phases: OrderedSet<String>,
    keywords: OrderedSet<String>,
    iuse: OrderedSet<String>,
    inherit: OrderedSet<String>,
    inherited: OrderedSet<String>,
}

macro_rules! split {
    ($s:expr) => {
        $s.split_whitespace().map(String::from)
    };
}

macro_rules! join {
    ($set:expr) => {{
        if $set.is_empty() {
            None
        } else {
            Some($set.iter().join(" "))
        }
    }};
}

impl Metadata {
    /// Convert raw metadata key value to stored value.
    fn convert(&mut self, eapi: &'static Eapi, key: Key, val: &str) -> crate::Result<()> {
        use Key::*;
        match key {
            DESCRIPTION => self.description = val.to_string(),
            SLOT => self.slot = val.to_string(),
            DEPEND | BDEPEND | IDEPEND | RDEPEND | PDEPEND => {
                if let Some(val) = dep::parse::dependencies(val, eapi)
                    .map_err(|e| Error::InvalidValue(format!("invalid {key}: {e}")))?
                {
                    self.deps.insert(key, val);
                }
            }
            LICENSE => self.license = dep::parse::license(val)?,
            PROPERTIES => self.properties = dep::parse::properties(val)?,
            REQUIRED_USE => self.required_use = dep::parse::required_use(val, eapi)?,
            RESTRICT => self.restrict = dep::parse::restrict(val)?,
            SRC_URI => self.src_uri = dep::parse::src_uri(val, eapi)?,
            HOMEPAGE => self.homepage = split!(val).collect(),
            DEFINED_PHASES => self.defined_phases = split!(val).sorted().collect(),
            KEYWORDS => self.keywords = split!(val).collect(),
            IUSE => self.iuse = split!(val).collect(),
            INHERIT => self.inherit = split!(val).collect(),
            INHERITED => self.inherited = split!(val).collect(),
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

    /// Deserialize a metadata string into [`Metadata`].
    pub(crate) fn deserialize(s: &str, eapi: &'static Eapi) -> crate::Result<Self> {
        let mut meta = Self::default();

        let iter = s
            .lines()
            .filter_map(|l| {
                l.split_once('=').map(|(s, v)| match (s, v) {
                    ("_eclasses_", v) => ("INHERITED", v),
                    // single hyphen means no phases are defined as per PMS
                    ("DEFINED_PHASES", "-") => ("DEFINED_PHASES", ""),
                    _ => (s, v),
                })
            })
            .filter_map(|(k, v)| Key::from_str(k).ok().map(|k| (k, v)))
            .filter(|(k, _)| eapi.metadata_keys().contains(k));

        for (key, val) in iter {
            if key == Key::INHERITED {
                meta.inherited = val
                    .split_whitespace()
                    .tuples()
                    .map(|(name, _chksum)| name.to_string())
                    .collect();
            } else {
                meta.convert(eapi, key, val)?;
            }
        }

        Ok(meta)
    }

    /// Serialize [`Metadata`] to the given package's metadata/md5-cache file in the related repo.
    pub(crate) fn serialize(pkg: &RawPkg) -> crate::Result<()> {
        // convert raw pkg into metadata via sourcing
        let meta: Metadata = pkg.try_into()?;

        // return the MD5 digest for a known eclass
        let eclass_digest = |name: &str| -> &str {
            pkg.repo()
                .eclasses()
                .get(name)
                .expect("missing eclass")
                .digest()
        };

        // convert metadata fields to metadata lines
        use Key::*;
        let mut data = Key::iter()
            .filter_map(|key| match key {
                DESCRIPTION => Some(key.line(&meta.description)),
                SLOT => Some(key.line(&meta.slot)),
                DEPEND | BDEPEND | IDEPEND | RDEPEND | PDEPEND => {
                    meta.deps.get(&key).map(|d| key.line(d))
                }
                LICENSE => meta.license.as_ref().map(|d| key.line(d)),
                PROPERTIES => meta.properties.as_ref().map(|d| key.line(d)),
                REQUIRED_USE => meta.required_use.as_ref().map(|d| key.line(d)),
                RESTRICT => meta.restrict.as_ref().map(|d| key.line(d)),
                SRC_URI => meta.src_uri.as_ref().map(|d| key.line(d)),
                HOMEPAGE => join!(&meta.homepage).map(|s| key.line(s)),
                DEFINED_PHASES => {
                    // PMS specifies if no phase functions are defined, a single hyphen is used.
                    if meta.defined_phases.is_empty() {
                        Some(key.line("-"))
                    } else {
                        Some(key.line(meta.defined_phases.iter().join(" ")))
                    }
                }
                KEYWORDS => join!(&meta.keywords).map(|s| key.line(s)),
                IUSE => join!(&meta.iuse).map(|s| key.line(s)),
                INHERIT => join!(&meta.inherit).map(|s| key.line(s)),
                INHERITED => {
                    if meta.inherited.is_empty() {
                        None
                    } else {
                        let data = meta
                            .inherited
                            .iter()
                            .flat_map(|s| [s.as_str(), eclass_digest(s)])
                            .join("\t");
                        Some(key.line(data))
                    }
                }
                EAPI => Some(key.line(pkg.eapi())),
            })
            .join("\n");

        // append ebuild hash
        data.push_str(&format!("\n_md5_={}\n", pkg.digest()));

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

        // write metadata entry to a temporary file
        let pf = pkg.pf();
        let path = dir.join(format!(".{pf}"));
        fs::write(&path, data)
            .map_err(|e| Error::IO(format!("failed writing metadata: {path}: {e}")))?;

        // atomically move it into place
        let new_path = dir.join(pf);
        fs::rename(&path, &new_path)
            .map_err(|e| Error::IO(format!("failed renaming metadata: {path} -> {new_path}: {e}")))
    }

    /// Load valid metadata entry from cache.
    pub(crate) fn load(cpv: &Cpv, repo: &Repo) -> crate::Result<String> {
        let path = repo.metadata().cache_path().join(cpv.to_string());
        let data = fs::read_to_string(&path).map_err(|e| {
            if e.kind() != io::ErrorKind::NotFound {
                warn!("error loading ebuild metadata: {path:?}: {e}");
            }
            Error::IO(format!("failed loading ebuild metadata: {path:?}: {e}"))
        })?;

        let pkg = RawPkg::new(cpv.clone(), repo)?;

        // pull ebuild and eclass hash lines which should always be the last two
        let mut iter = data.lines().rev();
        let (ebuild_hash, eclasses) = match (iter.next(), iter.next()) {
            (Some(s1), Some(s2)) => (s1, s2),
            _ => return Err(Error::InvalidValue("missing required metadata".to_string())),
        };

        // verify ebuild hash
        if let Some(s) = ebuild_hash.strip_prefix("_md5_=") {
            if s != pkg.digest() {
                return Err(Error::InvalidValue("mismatched ebuild metadata digest".to_string()));
            }
        } else {
            return Err(Error::InvalidValue("missing ebuild metadata digest".to_string()));
        }

        // verify eclass hashes
        if let Some(s) = eclasses.strip_prefix("_eclasses_=") {
            if !s.split_whitespace().tuples().all(|(name, digest)| {
                repo.eclasses()
                    .get(name)
                    .map_or(false, |e| e.digest() == digest)
            }) {
                return Err(Error::InvalidValue("mismatched eclass metadata digest".to_string()));
            }
        }

        Ok(data)
    }

    pub(crate) fn description(&self) -> &str {
        &self.description
    }

    pub(crate) fn slot(&self) -> &str {
        let s = self.slot.as_str();
        s.split_once('/').map_or(s, |x| x.0)
    }

    pub(crate) fn subslot(&self) -> Option<&str> {
        let s = self.slot.as_str();
        s.split_once('/').map(|x| x.1)
    }

    pub(crate) fn deps(&self, key: Key) -> Option<&DepSet<Dep>> {
        self.deps.get(&key)
    }

    pub(crate) fn license(&self) -> Option<&DepSet<String>> {
        self.license.as_ref()
    }

    pub(crate) fn properties(&self) -> Option<&DepSet<String>> {
        self.properties.as_ref()
    }

    pub(crate) fn required_use(&self) -> Option<&DepSet<String>> {
        self.required_use.as_ref()
    }

    pub(crate) fn restrict(&self) -> Option<&DepSet<String>> {
        self.restrict.as_ref()
    }

    pub(crate) fn src_uri(&self) -> Option<&DepSet<Uri>> {
        self.src_uri.as_ref()
    }

    pub(crate) fn homepage(&self) -> &OrderedSet<String> {
        &self.homepage
    }

    pub(crate) fn defined_phases(&self) -> &OrderedSet<String> {
        &self.defined_phases
    }

    pub(crate) fn keywords(&self) -> &OrderedSet<String> {
        &self.keywords
    }

    pub(crate) fn iuse(&self) -> &OrderedSet<String> {
        &self.iuse
    }

    pub(crate) fn inherit(&self) -> &OrderedSet<String> {
        &self.inherit
    }

    pub(crate) fn inherited(&self) -> &OrderedSet<String> {
        &self.inherited
    }
}

impl TryFrom<&RawPkg<'_>> for Metadata {
    type Error = Error;

    fn try_from(pkg: &RawPkg) -> crate::Result<Self> {
        // TODO: run sourcing via an external process pool returning the requested variables
        pkg.source()?;

        let eapi = pkg.eapi();
        let build = get_build_mut();
        let mut meta = Self::default();

        // pull metadata values from shell variables
        let mut missing = vec![];
        for key in eapi.metadata_keys() {
            if let Some(val) = key.get(build, eapi) {
                meta.convert(eapi, *key, &val)?;
            } else if eapi.mandatory_keys().contains(key) {
                missing.push(key.as_ref());
            }
        }

        if !missing.is_empty() {
            missing.sort();
            let keys = missing.join(", ");
            return Err(Error::InvalidValue(format!("missing required values: {keys}")));
        }

        Ok(meta)
    }
}
