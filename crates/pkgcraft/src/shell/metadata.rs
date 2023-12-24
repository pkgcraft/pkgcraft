use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};

use camino::Utf8Path;
use itertools::Itertools;
use scallop::{functions, variables};
use strum::{AsRefStr, Display, EnumString};
use tracing::warn;

use crate::dep::{self, Cpv, Dep, DependencySet, Slot, Uri};
use crate::eapi::Eapi;
use crate::pkg::ebuild::{iuse::Iuse, keyword::Keyword};
use crate::pkg::{ebuild::raw::Pkg, Package, RepoPackage, Source};
use crate::repo::ebuild::{Eclass, Repo};
use crate::types::OrderedSet;
use crate::Error;

use super::get_build_mut;
use super::phase::Phase;

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

#[derive(Debug, Default)]
pub(crate) struct Metadata<'a> {
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
    fn deserialize(
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
                    .map(eclass)
                    .collect::<crate::Result<OrderedSet<_>>>()?
            }
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

    /// Serialize a ebuild package's metadata to its raw form.
    pub(crate) fn serialize(pkg: &Pkg) -> crate::Result<Vec<u8>> {
        // convert raw pkg into metadata via sourcing
        let meta: Metadata = pkg.try_into()?;
        let eapi = pkg.eapi();

        // convert metadata fields to metadata lines
        use Key::*;
        let mut data = vec![];
        for key in eapi.metadata_keys() {
            match key {
                CHKSUM => writeln!(&mut data, "_md5_={}", meta.chksum)?,
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
                        let val = meta.defined_phases.iter().map(|p| p.name()).join(" ");
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
                            .flat_map(|e| [e.name(), e.chksum()])
                            .join("\t");
                        writeln!(&mut data, "_eclasses_={val}")?;
                    }
                }
                EAPI => writeln!(&mut data, "{key}={eapi}")?,
            }
        }

        Ok(data)
    }

    /// Verify a metadata entry is valid using its checksum values.
    pub(crate) fn verify(cpv: &Cpv<String>, repo: &'a Repo, cache_path: &Utf8Path) -> bool {
        Pkg::try_new(cpv.clone(), repo)
            .map(|p| Self::load(&p, cache_path, false).is_err())
            .unwrap_or_default()
    }

    /// Deserialize a metadata entry for a given package into [`Metadata`].
    pub(crate) fn load(
        pkg: &Pkg<'a>,
        cache_path: &Utf8Path,
        deserialize: bool,
    ) -> crate::Result<Self> {
        let eapi = pkg.eapi();
        let repo = pkg.repo();

        let path = cache_path.join(pkg.cpv().to_string());
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
                let Some(eclass) = repo.eclasses().get(name) else {
                    return Err(Error::InvalidValue(format!("nonexistent eclass: {name}")));
                };

                if eclass.chksum() != chksum {
                    return Err(Error::InvalidValue(format!("mismatched eclass checksum: {name}")));
                }

                if deserialize {
                    meta.inherited.insert(eclass);
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
                meta.deserialize(eapi, repo, &key, val)?;
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
    use crate::test::TEST_DATA;

    use super::*;

    #[test]
    fn load() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        let cache_path = repo.metadata().cache_path();
        for pkg in repo.iter_raw() {
            let r = Metadata::load(&pkg, cache_path, true);
            assert!(r.is_ok(), "{pkg}: failed metadata load: {}", r.unwrap_err());
        }
    }
}
