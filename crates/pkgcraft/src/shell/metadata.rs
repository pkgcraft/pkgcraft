use itertools::Itertools;
use scallop::{functions, variables};
use strum::{AsRefStr, Display, EnumIter, EnumString};

use crate::dep::{self, Dep, DependencySet, Slot, Uri};
use crate::eapi::Eapi;
use crate::pkg::ebuild::{iuse::Iuse, keyword::Keyword};
use crate::pkg::{ebuild::raw::Pkg, Package, RepoPackage, Source};
use crate::repo::ebuild::{Eclass, Repo};
use crate::types::OrderedSet;
use crate::Error;

use super::get_build_mut;
use super::phase::Phase;

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

/// Deserialized package metadata.
#[derive(Debug, Default)]
pub struct Metadata<'a> {
    eapi: &'static Eapi,
    description: String,
    slot: Slot<String>,
    bdepend: DependencySet<String, Dep<String>>,
    depend: DependencySet<String, Dep<String>>,
    idepend: DependencySet<String, Dep<String>>,
    pdepend: DependencySet<String, Dep<String>>,
    rdepend: DependencySet<String, Dep<String>>,
    license: DependencySet<String, String>,
    properties: DependencySet<String, String>,
    required_use: DependencySet<String, String>,
    restrict: DependencySet<String, String>,
    src_uri: DependencySet<String, Uri>,
    homepage: OrderedSet<String>,
    defined_phases: OrderedSet<&'a Phase>,
    keywords: OrderedSet<Keyword<String>>,
    iuse: OrderedSet<Iuse<String>>,
    inherit: OrderedSet<&'a Eclass>,
    inherited: OrderedSet<&'a Eclass>,
    chksum: String,
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
        // return the Eclass for a given identifier if it exists
        let eclass = |name: &str| -> crate::Result<&Eclass> {
            repo.eclasses()
                .get(name)
                .ok_or_else(|| Error::InvalidValue(format!("nonexistent eclass: {name}")))
        };

        // return the Keyword for a given identifier if it exists
        let keyword = |s: &str| -> crate::Result<Keyword<String>> {
            let keyword = Keyword::try_new(s)?;
            let arch = keyword.arch();
            if arch != "*" && !repo.arches().contains(arch) {
                Err(Error::InvalidValue(format!("nonexistent arch: {arch}")))
            } else {
                Ok(keyword)
            }
        };

        // return the Phase for a given name if it exists
        let phase = |name: &str| -> crate::Result<&Phase> {
            eapi.phases()
                .get(name)
                .ok_or_else(|| Error::InvalidValue(format!("nonexistent phase: {name}")))
        };

        use Key::*;
        match key {
            CHKSUM => self.chksum = val.to_string(),
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
            REQUIRED_USE => self.required_use = dep::parse::required_use_dependency_set(val, eapi)?,
            RESTRICT => self.restrict = dep::parse::restrict_dependency_set(val)?,
            SRC_URI => self.src_uri = dep::parse::src_uri_dependency_set(val, eapi)?,
            HOMEPAGE => self.homepage = val.split_whitespace().map(String::from).collect(),
            DEFINED_PHASES => {
                self.defined_phases = val
                    .split_whitespace()
                    .map(phase)
                    .collect::<crate::Result<OrderedSet<_>>>()?
            }
            KEYWORDS => {
                self.keywords = val
                    .split_whitespace()
                    .map(keyword)
                    .collect::<crate::Result<OrderedSet<_>>>()?
            }
            IUSE => {
                self.iuse = val
                    .split_whitespace()
                    .map(Iuse::try_new)
                    .collect::<crate::Result<OrderedSet<_>>>()?
            }
            INHERIT => {
                self.inherit = val
                    .split_whitespace()
                    .map(eclass)
                    .collect::<crate::Result<OrderedSet<_>>>()?
            }
            INHERITED => {
                self.inherited = val
                    .split_whitespace()
                    .tuples()
                    .map(|(name, _chksum)| eclass(name))
                    .collect::<crate::Result<OrderedSet<_>>>()?
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

    /// Serialize a metadata field to its string value, returning None for empty strings.
    pub(crate) fn serialize(&self, key: &Key) -> Option<String> {
        use Key::*;
        let value = match key {
            CHKSUM => self.chksum.clone(),
            DESCRIPTION => self.description.clone(),
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
            DEFINED_PHASES => self.defined_phases.iter().map(|p| p.name()).join(" "),
            KEYWORDS => self.keywords.iter().join(" "),
            IUSE => self.iuse.iter().join(" "),
            INHERIT => self.inherit.iter().join(" "),
            INHERITED => self
                .inherited
                .iter()
                .flat_map(|e| [e.name(), e.chksum()])
                .join("\t"),
            EAPI => self.eapi.to_string(),
        };

        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    pub(crate) fn eapi(&self) -> &'static Eapi {
        self.eapi
    }

    pub(crate) fn description(&self) -> &str {
        &self.description
    }

    pub(crate) fn slot(&self) -> &Slot<String> {
        &self.slot
    }

    pub(crate) fn bdepend(&self) -> &DependencySet<String, Dep<String>> {
        &self.bdepend
    }

    pub(crate) fn depend(&self) -> &DependencySet<String, Dep<String>> {
        &self.depend
    }

    pub(crate) fn idepend(&self) -> &DependencySet<String, Dep<String>> {
        &self.idepend
    }

    pub(crate) fn pdepend(&self) -> &DependencySet<String, Dep<String>> {
        &self.pdepend
    }

    pub(crate) fn rdepend(&self) -> &DependencySet<String, Dep<String>> {
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

    pub(crate) fn defined_phases(&self) -> &OrderedSet<&Phase> {
        &self.defined_phases
    }

    pub(crate) fn keywords(&self) -> &OrderedSet<Keyword<String>> {
        &self.keywords
    }

    pub(crate) fn iuse(&self) -> &OrderedSet<Iuse<String>> {
        &self.iuse
    }

    pub(crate) fn inherit(&self) -> &OrderedSet<&Eclass> {
        &self.inherit
    }

    pub(crate) fn inherited(&self) -> &OrderedSet<&Eclass> {
        &self.inherited
    }

    pub(crate) fn chksum(&self) -> &str {
        &self.chksum
    }
}

impl<'a> TryFrom<&Pkg<'a>> for Metadata<'a> {
    type Error = Error;

    fn try_from(pkg: &Pkg<'a>) -> crate::Result<Self> {
        // TODO: run sourcing via an external process pool returning the requested variables
        pkg.source()?;

        let eapi = pkg.eapi();
        let repo = pkg.repo();
        let build = get_build_mut();
        let mut meta = Self::default();

        // populate metadata fields using the current build state
        use Key::*;
        for key in eapi.metadata_keys() {
            match key {
                CHKSUM => meta.chksum = pkg.chksum().to_string(),
                DEFINED_PHASES => {
                    meta.defined_phases = eapi
                        .phases()
                        .iter()
                        .filter(|p| functions::find(p).is_some())
                        .collect();
                }
                INHERIT => meta.inherit = build.inherit.iter().copied().collect(),
                INHERITED => meta.inherited = build.inherited.iter().copied().collect(),
                key => {
                    if let Some(val) = build.incrementals.get(key) {
                        let s = val.iter().join(" ");
                        meta.deserialize(eapi, repo, key, &s)?;
                    } else if let Some(val) = variables::optional(key) {
                        let s = val.split_whitespace().join(" ");
                        meta.deserialize(eapi, repo, key, &s)?;
                    } else if eapi.mandatory_keys().contains(key) {
                        return Err(Error::InvalidValue(format!("missing required value: {key}")));
                    }
                }
            }
        }

        Ok(meta)
    }
}

#[cfg(test)]
mod tests {
    use crate::shell::BuildData;
    use crate::test::TEST_DATA;

    use super::*;

    #[test]
    fn try_from_raw_pkg() {
        // valid
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        for pkg in repo.iter_raw() {
            BuildData::from_raw_pkg(&pkg);
            let r = Metadata::try_from(&pkg);
            assert!(r.is_ok(), "{pkg}: failed metadata serialization: {}", r.unwrap_err());
        }

        // invalid
        let repo = TEST_DATA.ebuild_repo("bad").unwrap();
        for pkg in repo.iter_raw() {
            BuildData::from_raw_pkg(&pkg);
            let r = Metadata::try_from(&pkg);
            assert!(r.is_err(), "{pkg}: didn't fail");
        }
    }
}
