use std::{fmt, fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use itertools::Itertools;
use tracing::warn;

use crate::files::atomic_write_file;
use crate::pkg::{ebuild::raw::Pkg, Package, RepoPackage};
use crate::shell::metadata::{Key, Metadata};
use crate::Error;

use super::{Cache, CacheEntry, CacheFormat};

/// Convert a metadata key from its raw, metadata file string value.
fn key_from_meta(s: &str) -> crate::Result<Key> {
    match s {
        "_eclasses_" => Ok(Key::INHERITED),
        "_md5_" => Ok(Key::CHKSUM),
        s => s
            .parse()
            .map_err(|_| Error::InvalidValue(format!("invalid metadata key: {s}"))),
    }
}

/// Convert a metadata key to its raw, metadata file string value.
fn key_to_meta(key: &Key) -> &str {
    match key {
        Key::INHERITED => "_eclasses_",
        Key::CHKSUM => "_md5_",
        key => key.as_ref(),
    }
}

#[derive(Debug, Default)]
pub struct Md5DictEntry(IndexMap<Key, String>);

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
                meta.deserialize(pkg.eapi(), pkg.repo(), key, val)?;
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

impl<S: AsRef<str>> From<S> for Md5DictEntry {
    fn from(value: S) -> Self {
        let data = value
            .as_ref()
            .lines()
            .filter_map(|l| l.split_once('='))
            .filter_map(|(s, v)| key_from_meta(s).ok().map(|k| (k, v.to_string())))
            .collect();

        Self(data)
    }
}

impl From<&Metadata<'_>> for Md5DictEntry {
    fn from(meta: &Metadata) -> Self {
        let data = meta
            .eapi()
            .metadata_keys()
            .iter()
            .filter_map(|key| {
                let value = meta.serialize(key);
                if !value.is_empty() {
                    Some((*key, value))
                } else {
                    None
                }
            })
            .collect();

        Self(data)
    }
}

impl fmt::Display for Md5DictEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (key, val) in &self.0 {
            writeln!(f, "{}={val}", key_to_meta(key))?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Md5Dict {
    path: Utf8PathBuf,
}

impl Md5Dict {
    /// Load a metadata cache from the default location.
    pub(super) fn repo(path: &Utf8Path) -> Self {
        Self {
            path: path.join("metadata/md5-cache"),
        }
    }

    /// Load a metadata cache from a custom location.
    pub(super) fn custom<P: AsRef<Utf8Path>>(path: P) -> Self {
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
                warn!("error loading ebuild metadata: {path:?}: {e}");
            }
            Error::IO(format!("failed loading ebuild metadata: {path:?}: {e}"))
        })?;

        let meta = Md5DictEntry::from(&data);
        meta.verify(pkg)?;
        Ok(meta)
    }

    fn update(&self, pkg: &Pkg, meta: &Metadata) -> crate::Result<()> {
        // determine metadata entry directory
        let dir = self.path.join(pkg.cpv().category());

        // convert pkg metadata to cache entry format
        let entry = Md5DictEntry::from(meta);

        // atomically create metadata file
        let pf = pkg.cpv().pf();
        let path = dir.join(format!(".{pf}"));
        let new_path = dir.join(pf);
        atomic_write_file(&path, entry.to_string(), &new_path)?;

        Ok(())
    }
}
