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
    pub(crate) eapi: &'static Eapi,
    pub(crate) description: String,
    pub(crate) slot: Slot<String>,
    pub(crate) bdepend: DependencySet<String, Dep<String>>,
    pub(crate) depend: DependencySet<String, Dep<String>>,
    pub(crate) idepend: DependencySet<String, Dep<String>>,
    pub(crate) pdepend: DependencySet<String, Dep<String>>,
    pub(crate) rdepend: DependencySet<String, Dep<String>>,
    pub(crate) license: DependencySet<String, String>,
    pub(crate) properties: DependencySet<String, String>,
    pub(crate) required_use: DependencySet<String, String>,
    pub(crate) restrict: DependencySet<String, String>,
    pub(crate) src_uri: DependencySet<String, Uri>,
    pub(crate) homepage: OrderedSet<String>,
    pub(crate) defined_phases: OrderedSet<&'a Phase>,
    pub(crate) keywords: OrderedSet<Keyword<String>>,
    pub(crate) iuse: OrderedSet<Iuse<String>>,
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
        let keyword = |s: &str| -> crate::Result<Keyword<String>> {
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
            REQUIRED_USE => self.required_use = dep::parse::required_use_dependency_set(val, eapi)?,
            RESTRICT => self.restrict = dep::parse::restrict_dependency_set(val)?,
            SRC_URI => self.src_uri = dep::parse::src_uri_dependency_set(val, eapi)?,
            HOMEPAGE => self.homepage = val.split_whitespace().map(String::from).collect(),
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
