use std::borrow::Borrow;
use std::str::FromStr;
use std::{fmt, fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use itertools::Itertools;
use tracing::warn;

use crate::files::atomic_write_file;
use crate::pkg::{ebuild::raw::Pkg, Package, RepoPackage};
use crate::repo::Repository;
use crate::shell::metadata::{Key, Metadata};
use crate::Error;

use super::{Cache, CacheEntry, CacheFormat, Repo};

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
struct Md5DictKey(Key);

impl Borrow<Key> for Md5DictKey {
    fn borrow(&self) -> &Key {
        &self.0
    }
}

impl FromStr for Md5DictKey {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let key = match s {
            "_eclasses_" => Key::INHERITED,
            "_md5_" => Key::CHKSUM,
            s => s
                .parse()
                .map_err(|_| Error::InvalidValue(format!("invalid md5-dict key: {s}")))?,
        };

        Ok(Md5DictKey(key))
    }
}

impl fmt::Display for Md5DictKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            Key::INHERITED => write!(f, "_eclasses_"),
            Key::CHKSUM => write!(f, "_md5_"),
            key => write!(f, "{key}"),
        }
    }
}

#[derive(Debug, Default)]
pub struct Md5DictEntry(IndexMap<Md5DictKey, String>);

impl CacheEntry for Md5DictEntry {
    fn deserialize<'a>(&self, pkg: &Pkg<'a>) -> crate::Result<Metadata<'a>> {
        let mut meta = Metadata::default();

        for key in pkg.eapi().mandatory_keys() {
            if !self.0.contains_key(key) {
                return Err(Error::InvalidValue(format!("missing required value: {key}")));
            }
        }

        for key in pkg.eapi().metadata_keys() {
            if let Some(val) = self.0.get(key) {
                // PMS specifies if no phase functions are defined, a single hyphen is used.
                if !(key == &Key::DEFINED_PHASES && val == "-") {
                    meta.deserialize(pkg.eapi(), pkg.repo(), key, val)?;
                }
            }
        }

        Ok(meta)
    }

    fn verify(&self, pkg: &Pkg) -> crate::Result<()> {
        // verify ebuild checksum
        if let Some(val) = self.0.get(&Key::CHKSUM) {
            if val != pkg.chksum() {
                return Err(Error::InvalidValue("mismatched ebuild checksum".to_string()));
            }
        } else {
            return Err(Error::InvalidValue("missing ebuild checksum".to_string()));
        }

        // verify eclass checksums
        if let Some(val) = self.0.get(&Key::INHERITED) {
            for (name, chksum) in val.split_whitespace().tuples() {
                let Some(eclass) = pkg.repo().eclasses().get(name) else {
                    return Err(Error::InvalidValue(format!("nonexistent eclass: {name}")));
                };

                if eclass.chksum() != chksum {
                    return Err(Error::InvalidValue(format!("mismatched eclass checksum: {name}")));
                }
            }
        }

        Ok(())
    }
}

impl Md5DictEntry {
    /// Serialize a cache entry to raw bytes for writing to a file.
    fn serialize(&self) -> Vec<u8> {
        self.0
            .iter()
            .flat_map(|(k, v)| format!("{k}={v}\n").into_bytes())
            .collect()
    }
}

impl FromIterator<(Md5DictKey, String)> for Md5DictEntry {
    fn from_iter<I: IntoIterator<Item = (Md5DictKey, String)>>(iterable: I) -> Self {
        Self(iterable.into_iter().collect())
    }
}

impl<S: AsRef<str>> From<S> for Md5DictEntry {
    fn from(value: S) -> Self {
        value
            .as_ref()
            .lines()
            .filter_map(|l| l.split_once('='))
            .filter_map(|(s, v)| s.parse().ok().map(|k| (k, v.to_string())))
            .collect()
    }
}

impl From<&Metadata<'_>> for Md5DictEntry {
    fn from(meta: &Metadata) -> Self {
        meta.eapi()
            .metadata_keys()
            .iter()
            .filter_map(|key| {
                // PMS specifies if no phase functions are defined, a single hyphen is used.
                let val = if key == &Key::DEFINED_PHASES && meta.defined_phases().is_empty() {
                    Some("-".to_string())
                } else {
                    meta.serialize(key)
                };
                val.map(|v| (Md5DictKey(*key), v))
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct Md5Dict {
    path: Utf8PathBuf,
}

impl Md5Dict {
    /// Load a metadata cache from the default repo location.
    pub(super) fn from_repo(repo: &Repo) -> Self {
        Self {
            path: repo.path().join("metadata/md5-cache"),
        }
    }

    /// Load a metadata cache from a custom location.
    pub(super) fn from_path<P: AsRef<Utf8Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl Cache for Md5Dict {
    type Entry = Md5DictEntry;

    fn format(&self) -> CacheFormat {
        CacheFormat::Md5Dict
    }

    fn path(&self) -> &Utf8Path {
        &self.path
    }

    fn get(&self, pkg: &Pkg) -> crate::Result<Self::Entry> {
        let path = self.path.join(pkg.cpv().to_string());
        let data = fs::read_to_string(&path).map_err(|e| {
            if e.kind() != io::ErrorKind::NotFound {
                warn!("error loading ebuild metadata: {path}: {e}");
            }
            Error::IO(format!("failed loading ebuild metadata: {path}: {e}"))
        })?;

        let meta = Md5DictEntry::from(&data);
        meta.verify(pkg)?;
        Ok(meta)
    }

    fn update(&self, pkg: &Pkg, meta: &Metadata) -> crate::Result<()> {
        // determine metadata entry directory
        let path = self.path.join(pkg.cpv().category());

        // convert pkg metadata to serialized cache entry format
        let data = Md5DictEntry::from(meta).serialize();

        // atomically create metadata file
        atomic_write_file(&path, &pkg.cpv().pf(), data)
    }

    fn remove(&self, _repo: &Repo) -> crate::Result<()> {
        let path = &self.path;
        fs::remove_dir_all(path)
            .map_err(|e| Error::IO(format!("failed removing metadata cache: {path}: {e}")))
    }
}
