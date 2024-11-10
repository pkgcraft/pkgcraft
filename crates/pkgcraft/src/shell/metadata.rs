use itertools::Itertools;
use scallop::{functions, variables};

use crate::pkg::ebuild::metadata::{Key, Metadata, MetadataRaw};
use crate::pkg::ebuild::raw::Pkg;
use crate::pkg::{Package, RepoPackage, Source};
use crate::Error;

use super::get_build_mut;

impl TryFrom<&Pkg> for MetadataRaw {
    type Error = Error;

    fn try_from(pkg: &Pkg) -> crate::Result<Self> {
        // TODO: run sourcing via an external process pool returning the requested variables
        pkg.source()?;

        // populate metadata fields with raw string values
        use Key::*;
        Ok(MetadataRaw(
            pkg.eapi()
                .metadata_keys()
                .iter()
                .filter_map(|key| match key {
                    CHKSUM | DEFINED_PHASES | INHERIT | INHERITED => None,
                    key => {
                        variables::optional(key).map(|val| (*key, val.split_whitespace().join(" ")))
                    }
                })
                .collect(),
        ))
    }
}

impl TryFrom<&Pkg> for Metadata {
    type Error = Error;

    fn try_from(pkg: &Pkg) -> crate::Result<Self> {
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
                        .copied()
                        .collect();
                }
                INHERIT => meta.inherit = build.inherit.clone(),
                INHERITED => meta.inherited = build.inherited.clone(),
                key => {
                    if let Some(val) = build.incrementals.get(key) {
                        let s = val.iter().join(" ");
                        meta.deserialize(eapi, &repo, key, &s)?;
                    } else if let Some(val) = variables::optional(key) {
                        let s = val.split_whitespace().join(" ");
                        meta.deserialize(eapi, &repo, key, &s)?;
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
