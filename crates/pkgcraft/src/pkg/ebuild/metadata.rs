use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use itertools::Itertools;
use strum::{AsRefStr, Display, EnumIter, EnumString, IntoEnumIterator};

use crate::Error;
use crate::dep::{Dep, DependencySet, Slot, Uri};
use crate::eapi::Eapi;
use crate::repo::ebuild::{EbuildRepo, Eclass};
use crate::shell::phase::PhaseKind;
use crate::types::OrderedSet;

use super::iuse::Iuse;
use super::keyword::Keyword;

/// Ebuild package metadata variants.
///
/// Many of these directly correspond to variables set in ebuilds or eclasses. See the related
/// metadata key sets in [`Eapi`] for EAPI support relating to incrementals, dependencies, and
/// mandatory settings.
#[derive(AsRefStr, EnumIter, EnumString, Display, Debug, Copy, Clone)]
#[strum(serialize_all = "UPPERCASE")]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub enum MetadataKey {
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

impl PartialEq for MetadataKey {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl Eq for MetadataKey {}

impl Ord for MetadataKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for MetadataKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for MetadataKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl Borrow<str> for MetadataKey {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

/// Ebuild package metadata.
///
/// This is created via deserializing metadata cache entries or pulled directly from the
/// environment after sourcing an ebuild.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Metadata {
    pub(crate) eapi: &'static Eapi,
    pub(crate) description: String,
    pub(crate) slot: Slot,
    pub(crate) bdepend: DependencySet<Dep>,
    pub(crate) depend: DependencySet<Dep>,
    pub(crate) idepend: DependencySet<Dep>,
    pub(crate) pdepend: DependencySet<Dep>,
    pub(crate) rdepend: DependencySet<Dep>,
    pub(crate) license: DependencySet<String>,
    pub(crate) properties: DependencySet<String>,
    pub(crate) required_use: DependencySet<String>,
    pub(crate) restrict: DependencySet<String>,
    pub(crate) src_uri: DependencySet<Uri>,
    pub(crate) homepage: OrderedSet<String>,
    pub(crate) defined_phases: OrderedSet<PhaseKind>,
    pub(crate) keywords: OrderedSet<Keyword>,
    pub(crate) iuse: OrderedSet<Iuse>,
    pub(crate) inherit: OrderedSet<Eclass>,
    pub(crate) inherited: OrderedSet<Eclass>,
    pub(crate) chksum: String,
}

impl Metadata {
    /// Return the iterator of metadata keys for the metadata object in cache entry order.
    pub(crate) fn keys(&self) -> impl Iterator<Item = MetadataKey> {
        MetadataKey::iter().filter(|x| self.eapi.metadata_keys().contains(x))
    }

    /// Deserialize a metadata string value to its field value.
    pub(crate) fn deserialize(
        &mut self,
        eapi: &'static Eapi,
        repo: &EbuildRepo,
        key: &MetadataKey,
        val: &str,
    ) -> crate::Result<()> {
        // return the Eclass for a given identifier if it exists
        let eclass = |name: &str| -> crate::Result<Eclass> {
            repo.eclasses()
                .get(name)
                .cloned()
                .ok_or_else(|| Error::InvalidValue(format!("nonexistent eclass: {name}")))
        };

        // return the Keyword for a given identifier if it exists
        let keyword = |s: &str| -> crate::Result<Keyword> {
            let keyword = Keyword::try_new(s)?;
            let arch = keyword.arch();
            if arch != "*" && !repo.arches().contains(arch) {
                Err(Error::InvalidValue(format!("nonexistent arch: {arch}")))
            } else {
                Ok(keyword)
            }
        };

        // return the Phase for a given name if it exists
        let phase = |name: &str| -> crate::Result<PhaseKind> {
            eapi.phases()
                .get(name)
                .map(|x| x.kind)
                .ok_or_else(|| Error::InvalidValue(format!("nonexistent phase: {name}")))
        };

        use MetadataKey::*;
        match key {
            CHKSUM => self.chksum = val.to_string(),
            DESCRIPTION => self.description = val.to_string(),
            SLOT => self.slot = Slot::try_new(val)?,
            BDEPEND => self.bdepend = DependencySet::package(val, eapi)?,
            DEPEND => self.depend = DependencySet::package(val, eapi)?,
            IDEPEND => self.idepend = DependencySet::package(val, eapi)?,
            PDEPEND => self.pdepend = DependencySet::package(val, eapi)?,
            RDEPEND => self.rdepend = DependencySet::package(val, eapi)?,
            LICENSE => self.license = DependencySet::license(val)?,
            PROPERTIES => self.properties = DependencySet::properties(val)?,
            REQUIRED_USE => self.required_use = DependencySet::required_use(val)?,
            RESTRICT => self.restrict = DependencySet::restrict(val)?,
            SRC_URI => self.src_uri = DependencySet::src_uri(val)?,
            HOMEPAGE => self.homepage = val.split_whitespace().map(String::from).collect(),
            DEFINED_PHASES => {
                // PMS specifies if no phase functions are defined, a single hyphen is used.
                if val != "-" {
                    self.defined_phases = val.split_whitespace().map(phase).try_collect()?
                }
            }
            KEYWORDS => self.keywords = val.split_whitespace().map(keyword).try_collect()?,
            IUSE => self.iuse = val.split_whitespace().map(Iuse::try_new).try_collect()?,
            INHERIT => self.inherit = val.split_whitespace().map(eclass).try_collect()?,
            INHERITED => {
                self.inherited = val
                    .split_whitespace()
                    .tuples()
                    .map(|(name, _chksum)| eclass(name))
                    .try_collect()?
            }
            EAPI => {
                let sourced: &Eapi = val.try_into()?;
                if sourced != eapi {
                    return Err(Error::InvalidValue(format!(
                        "mismatched sourced and parsed EAPIs: {sourced} != {eapi}"
                    )));
                }
                self.eapi = eapi;
            }
        }

        Ok(())
    }

    /// Serialize a metadata field to its string value.
    pub(crate) fn serialize(&self, key: MetadataKey) -> String {
        use MetadataKey::*;
        match key {
            CHKSUM => self.chksum.to_string(),
            DESCRIPTION => self.description.to_string(),
            SLOT => self.slot.to_string(),
            BDEPEND => self.bdepend.to_string(),
            DEPEND => self.depend.to_string(),
            IDEPEND => self.idepend.to_string(),
            PDEPEND => self.pdepend.to_string(),
            RDEPEND => self.rdepend.to_string(),
            LICENSE => self.license.to_string(),
            PROPERTIES => self.properties.to_string(),
            REQUIRED_USE => self.required_use.to_string(),
            RESTRICT => self.restrict.to_string(),
            SRC_URI => self.src_uri.to_string(),
            HOMEPAGE => self.homepage.iter().join(" "),
            DEFINED_PHASES => {
                // PMS specifies if no phase functions are defined, a single hyphen is used.
                if self.defined_phases.is_empty() {
                    "-".to_string()
                } else {
                    self.defined_phases.iter().map(|p| p.name()).join(" ")
                }
            }
            KEYWORDS => self.keywords.iter().join(" "),
            IUSE => self.iuse.iter().join(" "),
            INHERIT => self.inherit.iter().join(" "),
            INHERITED => self
                .inherited
                .iter()
                .flat_map(|e| [e.name(), e.chksum()])
                .join("\t"),
            EAPI => self.eapi.to_string(),
        }
    }
}
