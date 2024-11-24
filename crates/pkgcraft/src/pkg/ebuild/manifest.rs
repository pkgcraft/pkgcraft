use std::collections::{hash_map, HashMap, HashSet};
use std::fs;
use std::hash::Hash;

use camino::Utf8Path;
use itertools::Itertools;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::macros::build_path;
use crate::traits::PkgCacheData;
use crate::types::OrderedSet;
use crate::utils::digest;
use crate::Error;

#[derive(Display, EnumString, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Copy, Clone)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum HashType {
    Blake2b,
    Blake3,
    Sha512,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Checksum {
    kind: HashType,
    value: String,
}

impl Checksum {
    pub(super) fn try_new(kind: &str, value: &str) -> crate::Result<Self> {
        let kind = kind
            .parse()
            .map_err(|_| Error::InvalidValue(format!("unknown checksum kind: {kind}")))?;
        Ok(Checksum { kind, value: value.to_string() })
    }

    /// Verify the checksum matches the given data.
    fn verify(&self, data: &[u8]) -> crate::Result<()> {
        let new = match self.kind {
            HashType::Blake2b => digest::<blake2::Blake2b512>(data),
            HashType::Blake3 => digest::<blake3::Hasher>(data),
            HashType::Sha512 => digest::<sha2::Sha512>(data),
        };

        if self.value != new {
            return Err(Error::InvalidValue(format!(
                "{} checksum failed: orig: {}, new: {new}",
                self.kind, self.value
            )));
        }

        Ok(())
    }
}

#[derive(
    Display, EnumString, EnumIter, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone,
)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ManifestType {
    Aux,
    Dist,
    Ebuild,
    Misc,
}

/// Package manifest contained in Manifest files as defined by GLEP 44.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone)]
pub struct ManifestFile {
    kind: ManifestType,
    name: String,
    size: u64,
    checksums: Vec<Checksum>,
}

impl ManifestFile {
    fn try_new(kind: ManifestType, name: &str, size: u64, chksums: &[&str]) -> crate::Result<Self> {
        let checksums: Vec<_> = chksums
            .iter()
            .tuples()
            .map(|(kind, val)| Checksum::try_new(kind, val))
            .try_collect()?;

        Ok(ManifestFile {
            kind,
            name: name.to_string(),
            size,
            checksums,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn checksums(&self) -> &[Checksum] {
        &self.checksums
    }

    pub fn verify(
        &self,
        required_hashes: &OrderedSet<HashType>,
        pkgdir: &Utf8Path,
        distdir: &Utf8Path,
    ) -> crate::Result<()> {
        let path = match self.kind {
            ManifestType::Aux => build_path!(pkgdir, "files", &self.name),
            ManifestType::Dist => distdir.join(&self.name),
            _ => pkgdir.join(&self.name),
        };
        let data =
            fs::read(&path).map_err(|e| Error::IO(format!("failed verifying: {path}: {e}")))?;

        self.checksums
            .iter()
            .filter(|c| required_hashes.contains(&c.kind))
            .try_for_each(|c| c.verify(&data))
    }
}

#[derive(Debug, Clone)]
pub struct Manifest {
    files: HashMap<ManifestType, HashSet<ManifestFile>>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            files: ManifestType::iter().map(|t| (t, HashSet::new())).collect(),
        }
    }
}

impl PkgCacheData for Manifest {
    const RELPATH: &'static str = "Manifest";

    fn parse(data: &str) -> crate::Result<Self> {
        let mut manifest = Self::default();

        for (i, line) in data.lines().enumerate() {
            let fields: Vec<_> = line.split_whitespace().collect();

            // verify manifest tokens include at least one checksum
            let (mtype, name, size, files) = match &fields[..] {
                [mtype, name, size, files @ ..] if !files.is_empty() && files.len() % 2 == 0 => {
                    (mtype, name, size, files)
                }
                _ => {
                    return Err(Error::InvalidValue(format!(
                        "line {}, invalid number of manifest tokens",
                        i + 1,
                    )));
                }
            };

            let kind = mtype
                .parse()
                .map_err(|_| Error::InvalidValue(format!("invalid manifest type: {mtype}")))?;
            let size = size
                .parse()
                .map_err(|e| Error::InvalidValue(format!("line {}, invalid size: {e}", i + 1)))?;
            manifest
                .files
                .entry(kind)
                .or_default()
                .insert(ManifestFile::try_new(kind, name, size, files)?);
        }

        if manifest.is_empty() {
            return Err(Error::InvalidValue("empty Manifest".to_string()));
        }

        Ok(manifest)
    }
}

impl Manifest {
    pub fn distfiles(&self) -> &HashSet<ManifestFile> {
        self.files
            .get(&ManifestType::Dist)
            .expect("invalid ManifestFile::default()")
    }

    pub fn is_empty(&self) -> bool {
        self.files.values().all(|s| s.is_empty())
    }

    pub fn verify(
        &self,
        required_hashes: &OrderedSet<HashType>,
        pkgdir: &Utf8Path,
        distdir: &Utf8Path,
    ) -> crate::Result<()> {
        self.into_iter()
            .try_for_each(|f| f.verify(required_hashes, pkgdir, distdir))
    }
}

impl<'a> IntoIterator for &'a Manifest {
    type Item = &'a ManifestFile;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: self.files.values().flatten(),
        }
    }
}

pub struct Iter<'a> {
    iter: std::iter::Flatten<hash_map::Values<'a, ManifestType, HashSet<ManifestFile>>>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a ManifestFile;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::repo::ebuild::EbuildRepoBuilder;
    use crate::test::assert_err_re;

    use super::*;

    #[test]
    fn distfile_verification() {
        let mut config = crate::config::Config::default();
        let temp = EbuildRepoBuilder::new().build().unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        config.finalize().unwrap();

        let manifest_hashes = &repo.metadata().config.manifest_hashes;
        let required_hashes = &repo.metadata().config.manifest_required_hashes;
        let tmpdir = tempdir().unwrap();
        let distdir: &Utf8Path = tmpdir.path().try_into().unwrap();

        // empty
        let r = Manifest::parse("");
        assert_err_re!(r, "empty Manifest");

        // missing distfile
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B a SHA512 b
        "#};
        let manifest = Manifest::parse(data).unwrap();
        let r = manifest.verify(required_hashes, distdir, distdir);
        assert_err_re!(r, "No such file or directory");

        // failing primary checksum
        fs::write(distdir.join("a.tar.gz"), "value").unwrap();
        let r = manifest.verify(required_hashes, distdir, distdir);
        assert_err_re!(r, "BLAKE2B checksum failed");

        // secondary checksum failure is ignored since it's not in manifest-required-hashes for the repo
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 b
        "#};
        let manifest = Manifest::parse(data).unwrap();
        assert!(manifest.verify(required_hashes, distdir, distdir).is_ok());

        // secondary checksum failure due to including it in the required hashes param
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 b
        "#};
        let manifest = Manifest::parse(data).unwrap();
        let r = manifest.verify(manifest_hashes, distdir, distdir);
        assert_err_re!(r, "SHA512 checksum failed");

        // verified
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 ec2c83edecb60304d154ebdb85bdfaf61a92bd142e71c4f7b25a15b9cb5f3c0ae301cfb3569cf240e4470031385348bc296d8d99d09e06b26f09591a97527296
        "#};
        let manifest = Manifest::parse(data).unwrap();
        assert!(manifest.verify(required_hashes, distdir, distdir).is_ok());
    }
}
