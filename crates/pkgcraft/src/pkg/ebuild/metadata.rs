use indexmap::IndexMap;
use itertools::Itertools;
use strum::{AsRefStr, Display, EnumIter, EnumString};

use crate::dep::{self, Dep, DependencySet, Slot, Uri};
use crate::eapi::Eapi;
use crate::repo::ebuild::{Eclass, Repo};
use crate::shell::phase::Phase;
use crate::types::OrderedSet;
use crate::Error;

use super::iuse::Iuse;
use super::keyword::Keyword;

/// Ebuild package metadata variants.
///
/// Many of these directly correspond to variables set in ebuilds or eclasses. See the related
/// metadata key sets in [`Eapi`] for EAPI support relating to incrementals, dependencies, and
/// mandatory settings.
#[derive(
    AsRefStr,
    EnumIter,
    EnumString,
    Display,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Copy,
    Clone,
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

/// Raw ebuild package metadata.
#[derive(Debug)]
pub struct MetadataRaw(pub(crate) IndexMap<Key, String>);

impl MetadataRaw {
    pub fn get(&self, key: &Key) -> Option<&String> {
        self.0.get(key)
    }
}

/// Ebuild package metadata.
///
/// This is created via deserializing metadata cache entries or pulled directly from the
/// environment after sourcing an ebuild.
#[derive(Debug, Default)]
pub struct Metadata<'a> {
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
    pub(crate) defined_phases: OrderedSet<&'a Phase>,
    pub(crate) keywords: OrderedSet<Keyword>,
    pub(crate) iuse: OrderedSet<Iuse>,
    pub(crate) inherit: OrderedSet<&'a Eclass>,
    pub(crate) inherited: OrderedSet<&'a Eclass>,
    pub(crate) chksum: String,
}

impl<'a> Metadata<'a> {
    /// Deserialize a metadata string value to its field value.
    pub(crate) fn deserialize(
        &mut self,
        eapi: &'static Eapi,
        repo: &'a Repo,
        key: &Key,
        val: &str,
    ) -> crate::Result<()> {
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

        use Key::*;
        match key {
            DESCRIPTION => self.description = val.to_string(),
            SLOT => self.slot = Slot::try_new(val)?,
            BDEPEND => self.bdepend = dep::parse::package_dependency_set(val, eapi)?,
            DEPEND => self.depend = dep::parse::package_dependency_set(val, eapi)?,
            IDEPEND => self.idepend = dep::parse::package_dependency_set(val, eapi)?,
            PDEPEND => self.pdepend = dep::parse::package_dependency_set(val, eapi)?,
            RDEPEND => self.rdepend = dep::parse::package_dependency_set(val, eapi)?,
            LICENSE => {
                self.license = dep::parse::license_dependency_set(val)?;
                for l in self.license.iter_flatten() {
                    if !repo.licenses().contains(l) {
                        return Err(Error::InvalidValue(format!("nonexistent license: {l}")));
                    }
                }
            }
            PROPERTIES => self.properties = dep::parse::properties_dependency_set(val)?,
            REQUIRED_USE => self.required_use = dep::parse::required_use_dependency_set(val)?,
            RESTRICT => self.restrict = dep::parse::restrict_dependency_set(val)?,
            SRC_URI => self.src_uri = dep::parse::src_uri_dependency_set(val)?,
            HOMEPAGE => self.homepage = val.split_whitespace().map(String::from).collect(),
            KEYWORDS => self.keywords = val.split_whitespace().map(keyword).try_collect()?,
            IUSE => self.iuse = val.split_whitespace().map(Iuse::try_new).try_collect()?,
            EAPI => {
                let sourced: &Eapi = val.try_into()?;
                if sourced != eapi {
                    return Err(Error::InvalidValue(format!(
                        "mismatched sourced and parsed EAPIs: {sourced} != {eapi}"
                    )));
                }
                self.eapi = eapi;
            }
            _ => panic!("{key} metadata deserialization should pull from build state"),
        }

        Ok(())
    }
}
