use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::{fmt, fs, io};

use camino::Utf8Path;
use indexmap::IndexSet;
use itertools::Itertools;
use rayon::prelude::*;
use strum::{Display, EnumIter, EnumString};

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

    fn value(&self, data: &str) -> crate::Result<String> {
        if data.chars().any(|c| !c.is_ascii_hexdigit()) {
            return Err(Error::InvalidValue(format!("invalid {self} hash: {data}")));
        }
        // TODO: verify data length
        Ok(data.to_string())
    }

    fn checksum(&self, data: &[u8]) -> Checksum {
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
        let kind: HashType = kind
            .parse()
            .map_err(|_| Error::InvalidValue(format!("unsupported hash: {kind}")))?;
        let value = kind.value(value)?;
        Ok(Checksum { kind, value })
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
    fn try_new(kind: ManifestType, name: &str, size: u64, hashes: &[&str]) -> crate::Result<Self> {
        let checksums: Vec<_> = hashes
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

    fn from_path<P: AsRef<Utf8Path>>(
        kind: ManifestType,
        path: P,
        hashes: &OrderedSet<HashType>,
    ) -> crate::Result<Self> {
        let path = path.as_ref();
        let data = fs::read(path)
            .map_err(|e| Error::InvalidValue(format!("failed reading file: {path}: {e}")))?;
        let name = path
            .file_name()
            .ok_or_else(|| Error::InvalidValue(format!("invalid file: {path}")))?;
        let checksums = hashes.iter().map(|kind| kind.checksum(&data)).collect();

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

#[derive(Debug, Default, Eq, PartialEq, Clone)]
pub struct Manifest(IndexSet<ManifestFile>);

impl PkgCacheData for Manifest {
    const RELPATH: &'static str = "Manifest";

    fn parse(data: &str) -> crate::Result<Self> {
        Self::parse(data)
    }
}

impl Manifest {
    /// Parse a [`Manifest`] from a file.
    pub fn from_path(path: &Utf8Path) -> crate::Result<Self> {
        match fs::read_to_string(path) {
            Ok(data) => Self::parse(&data)
                .map_err(|e| Error::InvalidValue(format!("invalid manifest: {path}: {e}"))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::IO(format!("failed reading manifest: {path}: {e}"))),
        }
    }

    /// Parse a string into a [`Manifest`].
    fn parse(data: &str) -> crate::Result<Self> {
        let mut manifest = Self::default();

        for (i, line) in data.lines().enumerate() {
            let fields: Vec<_> = line.split_whitespace().collect();

            // verify manifest tokens include at least one hash
            let (mtype, name, size, hashes) = match &fields[..] {
                [mtype, name, size, hashes @ ..] if !hashes.is_empty() && hashes.len() % 2 == 0 => {
                    (mtype, name, size, hashes)
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
                .insert(ManifestFile::try_new(kind, name, size, hashes)?);
        }

        if manifest.is_empty() {
            return Err(Error::InvalidValue("empty Manifest".to_string()));
        }

        Ok(manifest)
    }

    pub fn get(&self, name: &str) -> Option<&ManifestFile> {
        self.0.get(name)
    }

    pub fn distfiles(&self) -> impl Iterator<Item = &ManifestFile> {
        self.0.iter().filter(|x| x.kind == ManifestType::Dist)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Update the [`Manifest`] entries relating to an iterator of paths.
    pub fn update<I, P>(&mut self, paths: I, repo: &EbuildRepo) -> crate::Result<()>
    where
        I: IntoParallelIterator<Item = P>,
        P: AsRef<Utf8Path>,
    {
        // TODO: support thick manifests
        if !repo.metadata().config.thin_manifests {
            return Err(Error::InvalidValue(
                "updating thick manifests isn't supported".to_string(),
            ));
        }

        let hashes = &repo.metadata().config.manifest_hashes;
        let new: Vec<_> = paths
            .into_par_iter()
            .map(|path| ManifestFile::from_path(ManifestType::Dist, path, hashes))
            .collect();
        for result in new {
            self.0.replace(result?);
        }
        self.0.par_sort();
        Ok(())
    }

    pub fn verify(&self, pkgdir: &Utf8Path, distdir: &Utf8Path) -> crate::Result<()> {
        self.into_iter().try_for_each(|f| f.verify(pkgdir, distdir))
    }
}

impl fmt::Display for Manifest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for file in &self.0 {
            writeln!(f, "{file}")?;
        }
        Ok(())
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
