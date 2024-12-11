use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::Write;

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Itertools;
use rayon::prelude::*;
use strum::{Display, EnumIter, EnumString};

use crate::dep::Uri;
use crate::macros::build_path;
use crate::repo::ebuild::EbuildRepo;
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

impl HashType {
    fn hash(&self, data: &[u8]) -> String {
        match self {
            HashType::Blake2b => digest::<blake2::Blake2b512>(data),
            HashType::Blake3 => digest::<blake3::Hasher>(data),
            HashType::Sha512 => digest::<sha2::Sha512>(data),
        }
    }

    fn to_checksum(&self, data: &[u8]) -> Checksum {
        Checksum {
            kind: *self,
            value: self.hash(data),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Checksum {
    kind: HashType,
    value: String,
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.kind, self.value)
    }
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
        let hash = self.kind.hash(data);

        if self.value != hash {
            return Err(Error::InvalidValue(format!(
                "{} checksum failed: expected: {}, got: {hash}",
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
#[derive(Debug, Clone)]
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

        Ok(Self {
            kind,
            name: name.to_string(),
            size,
            checksums,
        })
    }

    fn from_path(
        kind: ManifestType,
        path: &Utf8Path,
        hashes: &OrderedSet<HashType>,
    ) -> crate::Result<Self> {
        let data = fs::read(path).unwrap();
        let name = path.file_name().unwrap();
        let checksums = hashes.iter().map(|kind| kind.to_checksum(&data)).collect();

        Ok(Self {
            kind,
            name: name.to_string(),
            size: data.len() as u64,
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

    pub fn verify(&self, pkgdir: &Utf8Path, distdir: &Utf8Path) -> crate::Result<()> {
        let name = self.name();
        let path = match self.kind {
            ManifestType::Aux => build_path!(pkgdir, "files", name),
            ManifestType::Dist => distdir.join(name),
            _ => pkgdir.join(name),
        };
        let data =
            fs::read(&path).map_err(|e| Error::IO(format!("failed reading: {path}: {e}")))?;

        self.checksums.iter().try_for_each(|c| {
            c.verify(&data)
                .map_err(|e| Error::InvalidValue(format!("failed verifying: {name}: {e}")))
        })
    }
}

impl PartialEq for ManifestFile {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for ManifestFile {}

impl Hash for ManifestFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Ord for ManifestFile {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind
            .cmp(&other.kind)
            .then_with(|| self.name.cmp(&other.name))
    }
}

impl PartialOrd for ManifestFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Borrow<str> for ManifestFile {
    fn borrow(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for ManifestFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let checksums = self.checksums.iter().join(" ");
        write!(f, "{} {} {} {}", self.kind, self.name, self.size, checksums)
    }
}

#[derive(Debug, Eq, PartialEq, Default, Clone)]
pub struct Manifest(IndexSet<ManifestFile>);

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
                .0
                .insert(ManifestFile::try_new(kind, name, size, files)?);
        }

        if manifest.is_empty() {
            return Err(Error::InvalidValue("empty Manifest".to_string()));
        }

        Ok(manifest)
    }
}

impl Manifest {
    pub fn get(&self, name: &str) -> Option<&ManifestFile> {
        self.0.get(name)
    }

    pub fn distfiles(&self) -> impl Iterator<Item = &ManifestFile> {
        self.0.iter().filter(|x| x.kind == ManifestType::Dist)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn update(
        &self,
        uris: &[Uri],
        pkgdir: &Utf8Path,
        distdir: &Utf8Path,
        repo: &EbuildRepo,
    ) -> crate::Result<()> {
        // TODO: support thick manifests
        let hashes = &repo.metadata().config.manifest_hashes;
        let mut files = self.0.clone();
        let new: Vec<_> = uris
            .into_par_iter()
            .map(|uri| {
                let path = distdir.join(uri.filename());
                ManifestFile::from_path(ManifestType::Dist, &path, hashes)
            })
            .collect();
        for result in new {
            files.insert(result?);
        }
        files.par_sort();
        let mut f = File::create(pkgdir.join("Manifest"))?;
        for file in files {
            writeln!(&mut f, "{file}")?;
        }
        Ok(())
    }

    pub fn verify(&self, pkgdir: &Utf8Path, distdir: &Utf8Path) -> crate::Result<()> {
        self.into_iter().try_for_each(|f| f.verify(pkgdir, distdir))
    }
}

impl<'a> IntoIterator for &'a Manifest {
    type Item = &'a ManifestFile;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.0.iter())
    }
}

/// An iterator over the entries of a [`Manifest`].
pub struct Iter<'a>(indexmap::set::Iter<'a, ManifestFile>);

impl<'a> Iterator for Iter<'a> {
    type Item = &'a ManifestFile;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::test::assert_err_re;

    use super::*;

    #[test]
    fn distfile_verification() {
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
        let r = manifest.verify(distdir, distdir);
        assert_err_re!(r, "No such file or directory");

        // primary checksum failure
        fs::write(distdir.join("a.tar.gz"), "value").unwrap();
        let r = manifest.verify(distdir, distdir);
        assert_err_re!(r, "BLAKE2B checksum failed");

        // secondary checksum failure
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 b
        "#};
        let manifest = Manifest::parse(data).unwrap();
        let r = manifest.verify(distdir, distdir);
        assert_err_re!(r, "SHA512 checksum failed");

        // verified
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 ec2c83edecb60304d154ebdb85bdfaf61a92bd142e71c4f7b25a15b9cb5f3c0ae301cfb3569cf240e4470031385348bc296d8d99d09e06b26f09591a97527296
        "#};
        let manifest = Manifest::parse(data).unwrap();
        assert!(manifest.verify(distdir, distdir).is_ok());
    }
}
