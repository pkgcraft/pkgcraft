use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use itertools::Itertools;
use strum::{AsRefStr, Display, EnumIter, EnumString};

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

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl Eq for Key {}

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

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl Borrow<str> for Key {
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
    /// Deserialize a metadata string value to its field value.
    pub(crate) fn deserialize(
        &mut self,
        eapi: &'static Eapi,
        repo: &EbuildRepo,
        key: &Key,
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

        match key {
            Key::CHKSUM => self.chksum = val.to_string(),
            Key::DESCRIPTION => self.description = val.to_string(),
            Key::SLOT => self.slot = Slot::try_new(val)?,
            Key::BDEPEND => self.bdepend = DependencySet::package(val, eapi)?,
            Key::DEPEND => self.depend = DependencySet::package(val, eapi)?,
            Key::IDEPEND => self.idepend = DependencySet::package(val, eapi)?,
            Key::PDEPEND => self.pdepend = DependencySet::package(val, eapi)?,
            Key::RDEPEND => self.rdepend = DependencySet::package(val, eapi)?,
            Key::LICENSE => self.license = DependencySet::license(val)?,
            Key::PROPERTIES => self.properties = DependencySet::properties(val)?,
            Key::REQUIRED_USE => self.required_use = DependencySet::required_use(val)?,
            Key::RESTRICT => self.restrict = DependencySet::restrict(val)?,
            Key::SRC_URI => self.src_uri = DependencySet::src_uri(val)?,
            Key::HOMEPAGE => {
                self.homepage = val.split_whitespace().map(String::from).collect()
            }
            Key::DEFINED_PHASES => {
                // PMS specifies if no phase functions are defined, a single hyphen is used.
                if val != "-" {
                    self.defined_phases = val.split_whitespace().map(phase).try_collect()?
                }
            }
            Key::KEYWORDS => {
                self.keywords = val.split_whitespace().map(keyword).try_collect()?
            }
            Key::IUSE => {
                self.iuse = val.split_whitespace().map(Iuse::try_new).try_collect()?
            }
            Key::INHERIT => self.inherit = val.split_whitespace().map(eclass).try_collect()?,
            Key::INHERITED => {
                self.inherited = val
                    .split_whitespace()
                    .tuples()
                    .map(|(name, _chksum)| eclass(name))
                    .try_collect()?
            }
            Key::EAPI => {
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
}
