use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::{fmt, fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use ordermap::OrderMap;
use rayon::prelude::*;
use strum::{Display, EnumIter, EnumString};

use crate::files::relative_paths;
use crate::macros::build_path;
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
    /// Generate a hash value from data.
    pub fn hash(&self, data: &[u8]) -> String {
        match self {
            HashType::Blake2b => digest::<blake2::Blake2b512>(data),
            HashType::Blake3 => digest::<blake3::Hasher>(data),
            HashType::Sha512 => digest::<sha2::Sha512>(data),
        }
    }

    /// Verify a hash value from a string.
    fn value(&self, data: &str) -> crate::Result<String> {
        if data.chars().any(|c| !c.is_ascii_hexdigit()) {
            return Err(Error::InvalidValue(format!("invalid {self} hash: {data}")));
        }
        // TODO: verify data length
        Ok(data.to_string())
    }

    /// Verify the hash matches the given data.
    fn verify(&self, data: &[u8], value: &str) -> crate::Result<()> {
        let hash = self.hash(data);

        if value != hash {
            return Err(Error::InvalidValue(format!(
                "{self} hash failed: expected: {value}, got: {hash}",
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
#[derive(Debug, Clone, Ord, PartialOrd)]
pub struct ManifestEntry {
    kind: ManifestType,
    name: String,
    size: u64,
    hashes: OrderMap<HashType, String>,
}

impl ManifestEntry {
    fn try_new(
        kind: ManifestType,
        name: &str,
        size: u64,
        data: &[&str],
    ) -> crate::Result<Self> {
        let mut hashes = OrderMap::new();
        for (kind, value) in data.iter().tuples() {
            let kind: HashType = kind
                .parse()
                .map_err(|_| Error::InvalidValue(format!("unsupported hash: {kind}")))?;
            let value = kind.value(value)?;
            hashes.insert(kind, value);
        }

        Ok(Self {
            kind,
            name: name.to_string(),
            size,
            hashes,
        })
    }

    fn from_path<'a, I, P, S>(
        kind: ManifestType,
        name: S,
        path: P,
        hashes: I,
    ) -> crate::Result<Self>
    where
        P: AsRef<Utf8Path>,
        I: IntoIterator<Item = &'a HashType>,
        S: fmt::Display,
    {
        let path = path.as_ref();
        let data = fs::read(path)
            .map_err(|e| Error::InvalidValue(format!("failed reading: {path}: {e}")))?;
        let hashes = hashes
            .into_iter()
            .map(|kind| (*kind, kind.hash(&data)))
            .collect();

        Ok(Self {
            kind,
            name: name.to_string(),
            size: data.len() as u64,
            hashes,
        })
    }

    pub fn kind(&self) -> ManifestType {
        self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn hashes(&self) -> &OrderMap<HashType, String> {
        &self.hashes
    }

    pub fn verify(&self, data: &[u8]) -> crate::Result<()> {
        let name = self.name();
        self.hashes.iter().try_for_each(|(hash, value)| {
            hash.verify(data, value).map_err(|e| {
                Error::InvalidValue(format!("{name}: failed verifying {hash}: {e}"))
            })
        })
    }
}

impl PartialEq for ManifestEntry {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for ManifestEntry {}

impl Hash for ManifestEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Borrow<str> for ManifestEntry {
    fn borrow(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for ManifestEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.kind, self.name, self.size)?;
        for (hash, value) in &self.hashes {
            write!(f, " {hash} {value}")?;
        }
        Ok(())
    }
}

impl FromStr for ManifestEntry {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let fields: Vec<_> = s.split_whitespace().collect();

        // verify manifest tokens include at least one hash
        let (mtype, name, size, hashes) = match &fields[..] {
            [mtype, name, size, hashes @ ..]
                if !hashes.is_empty() && hashes.len() % 2 == 0 =>
            {
                (mtype, name, size, hashes)
            }
            _ => {
                return Err(Error::InvalidValue(
                    "invalid number of manifest tokens".to_string(),
                ));
            }
        };

        let kind = mtype
            .parse()
            .map_err(|_| Error::InvalidValue(format!("invalid manifest type: {mtype}")))?;
        let size = size
            .parse()
            .map_err(|e| Error::InvalidValue(format!("invalid size: {e}")))?;

        Self::try_new(kind, name, size, hashes)
    }
}

#[derive(Debug, Default, Eq, PartialEq, Clone)]
pub struct Manifest(IndexSet<ManifestEntry>);

impl Manifest {
    /// Parse a [`Manifest`] from a file.
    pub(crate) fn from_path(path: &Utf8Path) -> crate::Result<Self> {
        match fs::read_to_string(path) {
            Ok(data) => Self::parse(&data)
                .map_err(|e| Error::InvalidValue(format!("failed parsing: {e}"))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::IO(format!("failed reading: {e}"))),
        }
    }

    /// Parse a string into a [`Manifest`].
    fn parse(data: &str) -> crate::Result<Self> {
        let mut manifest = Self::default();

        for (i, line) in data.lines().enumerate() {
            let entry: ManifestEntry = line
                .parse()
                .map_err(|e| Error::InvalidValue(format!("line {}: {e}", i + 1)))?;
            manifest.0.insert(entry);
        }

        if manifest.is_empty() {
            return Err(Error::InvalidValue("empty Manifest".to_string()));
        }

        Ok(manifest)
    }

    pub fn get(&self, name: &str) -> Option<&ManifestEntry> {
        self.0.get(name)
    }

    pub fn iter(&self) -> Iter {
        self.into_iter()
    }

    pub fn distfiles(&self) -> impl Iterator<Item = &ManifestEntry> {
        self.into_iter().filter(|x| x.kind == ManifestType::Dist)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return true if the [`Manifest`] contains thick entries, false otherwise.
    pub fn is_thick(&self) -> bool {
        self.0.iter().any(|x| x.kind() == ManifestType::Ebuild)
    }

    /// Determine if a Manifest is outdated.
    pub fn outdated(
        &self,
        pkgdir: &Utf8Path,
        distfiles: &IndexMap<String, (Utf8PathBuf, bool)>,
        thick: bool,
    ) -> bool {
        let mut files: IndexSet<_> = distfiles.keys().map(|s| s.to_string()).collect();
        if thick {
            files.extend(
                relative_paths(pkgdir)
                    .filter_map(|path| Utf8PathBuf::from_path_buf(path).ok())
                    .map(|path| path.into()),
            );
        }

        files.len() != self.0.len()
            || files.iter().any(|x| self.get(x).is_none())
            || distfiles.values().any(|(_, regen)| *regen)
    }

    /// Update the [`Manifest`] entries relating to an iterator of distfile paths.
    pub fn update<'a, I>(
        &mut self,
        distfiles: &IndexMap<String, (Utf8PathBuf, bool)>,
        hashes: I,
        pkgdir: &Utf8Path,
        thick: bool,
    ) -> crate::Result<()>
    where
        I: IntoIterator<Item = &'a HashType> + Send + Sync + Copy,
    {
        // generate distfile hashes
        let mut files: Vec<_> = distfiles
            .into_par_iter()
            .filter(|(_, (_, update))| *update)
            .map(|(name, (path, _))| {
                ManifestEntry::from_path(ManifestType::Dist, name, path, hashes)
            })
            .collect();

        // generate file hashes for thick manifests
        let files_path = pkgdir.join("files");
        if thick {
            // add files dir entries
            files.par_extend(
                relative_paths(&files_path)
                    .collect::<Vec<_>>()
                    .into_par_iter()
                    .filter_map(|path| Utf8PathBuf::from_path_buf(path).ok())
                    .map(|path| {
                        let abspath = files_path.join(&path);
                        ManifestEntry::from_path(ManifestType::Aux, path, abspath, hashes)
                    }),
            );

            let pkg_dir_files = pkgdir
                .read_dir_utf8()
                .map_err(|e| Error::IO(format!("failed reading package dir: {e}")))?;

            // add package dir entries
            files.par_extend(
                pkg_dir_files
                    .filter_map(Result::ok)
                    .filter(|e| e.path().is_file())
                    .filter(|e| e.file_name() != "Manifest")
                    .collect::<Vec<_>>()
                    .into_par_iter()
                    .map(|e| {
                        let ext = e.path().extension().unwrap_or_default();
                        let kind = if ext == "ebuild" {
                            ManifestType::Ebuild
                        } else {
                            ManifestType::Misc
                        };
                        let name = e.file_name();
                        let abspath = pkgdir.join(e.path());
                        ManifestEntry::from_path(kind, name, abspath, hashes)
                    }),
            );
        } else {
            // remove thick entries
            self.0.retain(|x| x.kind == ManifestType::Dist);
        }

        // replace matching entries with newly hashed values
        for result in files {
            self.0.replace(result?);
        }

        // remove entries for nonexistent files
        self.0.retain(|entry| {
            let name = entry.name();
            match entry.kind() {
                ManifestType::Aux => files_path.join(name).exists(),
                ManifestType::Ebuild | ManifestType::Misc => pkgdir.join(name).exists(),
                ManifestType::Dist => distfiles.contains_key(name),
            }
        });

        // sort manifest entries
        self.0.par_sort();

        Ok(())
    }

    pub fn verify(&self, pkgdir: &Utf8Path, distdir: &Utf8Path) -> crate::Result<()> {
        self.into_iter().try_for_each(|f| {
            let path = match f.kind {
                ManifestType::Aux => build_path!(pkgdir, "files", f.name()),
                ManifestType::Dist => distdir.join(f.name()),
                _ => pkgdir.join(f.name()),
            };
            let data =
                fs::read(&path).map_err(|e| Error::IO(format!("failed reading: {e}")))?;
            f.verify(&data)
        })
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
    type Item = &'a ManifestEntry;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.0.iter())
    }
}

/// An iterator over the entries of a [`Manifest`].
pub struct Iter<'a>(indexmap::set::Iter<'a, ManifestEntry>);

impl<'a> Iterator for Iter<'a> {
    type Item = &'a ManifestEntry;

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

        // primary hash failure
        fs::write(distdir.join("a.tar.gz"), "value").unwrap();
        let r = manifest.verify(distdir, distdir);
        assert_err_re!(r, "BLAKE2B hash failed");

        // secondary hash failure
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 b
        "#};
        let manifest = Manifest::parse(data).unwrap();
        let r = manifest.verify(distdir, distdir);
        assert_err_re!(r, "SHA512 hash failed");

        // verified
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 ec2c83edecb60304d154ebdb85bdfaf61a92bd142e71c4f7b25a15b9cb5f3c0ae301cfb3569cf240e4470031385348bc296d8d99d09e06b26f09591a97527296
        "#};
        let manifest = Manifest::parse(data).unwrap();
        assert!(manifest.verify(distdir, distdir).is_ok());
    }

    #[test]
    fn is_thick() {
        // thin
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 ec2c83edecb60304d154ebdb85bdfaf61a92bd142e71c4f7b25a15b9cb5f3c0ae301cfb3569cf240e4470031385348bc296d8d99d09e06b26f09591a97527296
        "#};
        let manifest = Manifest::parse(data).unwrap();
        assert!(!manifest.is_thick());

        // thick
        let data = indoc::indoc! {r#"
            DIST a.tar.gz 1 BLAKE2B 631ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 ec2c83edecb60304d154ebdb85bdfaf61a92bd142e71c4f7b25a15b9cb5f3c0ae301cfb3569cf240e4470031385348bc296d8d99d09e06b26f09591a97527296
            EBUILD a-1.ebuild 100 BLAKE2B 531ad87bd3f552d3454be98da63b68d13e55fad21cad040183006b52fce5ceeaf2f0178b20b3966447916a330930a8754c2ef1eed552e426a7e158f27a4668c5 SHA512 ac2c83edecb60304d154ebdb85bdfaf61a92bd142e71c4f7b25a15b9cb5f3c0ae301cfb3569cf240e4470031385348bc296d8d99d09e06b26f09591a97527296
        "#};
        let manifest = Manifest::parse(data).unwrap();
        assert!(manifest.is_thick());
    }
}
