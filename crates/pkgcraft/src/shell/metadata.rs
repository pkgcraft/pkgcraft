use itertools::Itertools;
use scallop::{functions, variables};

use crate::pkg::ebuild::metadata::{Key, Metadata};
use crate::pkg::ebuild::EbuildRawPkg;
use crate::pkg::{Package, RepoPackage, Source};
use crate::Error;

use super::get_build_mut;

/// WARNING: This should always be run via a standalone process as it alters the environment and is
/// not thread friendly in any fashion.
impl TryFrom<&EbuildRawPkg> for Metadata {
    type Error = Error;

    fn try_from(pkg: &EbuildRawPkg) -> crate::Result<Self> {
        pkg.source()?;

        let eapi = pkg.eapi();
        let repo = &pkg.repo();
        let build = get_build_mut();
        let mut meta = Self::default();

        // populate metadata fields using the current build state
        for key in eapi.metadata_keys() {
            match key {
                Key::CHKSUM => meta.deserialize(eapi, repo, key, pkg.chksum())?,
                Key::DEFINED_PHASES => {
                    meta.defined_phases = eapi
                        .phases()
                        .iter()
                        .filter(|p| functions::find(p).is_some())
                        .map(|p| p.kind)
                        .collect();
                }
                Key::INHERIT => meta.inherit = build.inherit.clone(),
                Key::INHERITED => meta.inherited = build.inherited.clone(),
                key => {
                    if let Some(val) = build.incrementals.get(key) {
                        let s = val.iter().join(" ");
                        meta.deserialize(eapi, repo, key, &s)?;
                    } else if let Some(val) = variables::optional(key) {
                        let s = val.split_whitespace().join(" ");
                        meta.deserialize(eapi, repo, key, &s)?;
                    } else if eapi.mandatory_keys().contains(key) {
                        return Err(Error::InvalidValue(format!(
                            "missing required value: {key}"
                        )));
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
    use crate::test::test_data;

    use super::*;

    #[test]
    fn try_from_raw_pkg() {
        // valid
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        for pkg in repo.iter_raw() {
            let pkg = pkg.unwrap();
            BuildData::from_raw_pkg(&pkg);
            let r = Metadata::try_from(&pkg);
            assert!(r.is_ok(), "{pkg}: failed metadata serialization: {}", r.unwrap_err());
        }

        // invalid
        let repo = data.ebuild_repo("bad").unwrap();
        // ignore pkgs with invalid EAPIs
        for pkg in repo.iter_raw().filter_map(Result::ok) {
            BuildData::from_raw_pkg(&pkg);
            let r = Metadata::try_from(&pkg);
            assert!(r.is_err(), "{pkg}: didn't fail");
        }
    }
}
