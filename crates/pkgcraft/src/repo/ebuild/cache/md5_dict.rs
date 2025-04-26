use std::borrow::Borrow;
use std::str::FromStr;
use std::{fmt, fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use itertools::Itertools;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::dep::Cpv;
use crate::files::{atomic_write_file, is_file};
use crate::pkg::ebuild::metadata::{Key, Metadata};
use crate::pkg::ebuild::EbuildRawPkg;
use crate::pkg::{Package, RepoPackage};
use crate::repo::EbuildRepo;
use crate::traits::Contains;
use crate::utils::digest;
use crate::Error;

use super::{Cache, CacheEntry, CacheFormat};

/// Wrapper that converts metadata keys to md5-dict compatible keys.
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
            s => s.parse().map_err(|_| {
                Error::InvalidValue(format!("invalid md5-dict cache key: {s}"))
            })?,
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

/// The format for md5-dict metadata cache entries.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Md5DictEntry(IndexMap<Md5DictKey, String>);

impl CacheEntry for Md5DictEntry {
    fn to_metadata(&self, pkg: &EbuildRawPkg) -> crate::Result<Metadata> {
        let mut meta = Metadata::default();
        let eapi = pkg.eapi();
        let repo = &pkg.repo();
        let invalid = |e| Error::InvalidValue(format!("{pkg}: invalid metadata: {e}"));

        for key in eapi.metadata_keys() {
            if let Some(val) = self.0.get(key) {
                meta.deserialize(eapi, repo, key, val)
                    .map_err(|e| invalid(e.to_string()))?;
            } else if eapi.mandatory_keys().contains(key) {
                return Err(invalid(format!("missing required value: {key}")));
            }
        }

        Ok(meta)
    }

    fn verify(&self, pkg: &EbuildRawPkg) -> crate::Result<()> {
        let invalid = |e| Error::InvalidValue(format!("{pkg}: invalid metadata: {e}"));

        // verify ebuild checksum
        if let Some(val) = self.0.get(&Key::CHKSUM) {
            if val != pkg.chksum() {
                return Err(invalid("mismatched ebuild checksum"));
            }
        } else {
            return Err(invalid("missing ebuild checksum"));
        }

        // verify eclass checksums
        if let Some(val) = self.0.get(&Key::INHERITED) {
            let repo = pkg.repo();
            for (name, chksum) in val.split_whitespace().tuples() {
                let Some(eclass) = repo.eclasses().get(name) else {
                    return Err(invalid(&format!("nonexistent eclass: {name}")));
                };

                if eclass.chksum() != chksum {
                    return Err(invalid(&format!("mismatched eclass checksum: {name}")));
                }
            }
        } else if self.0.get(&Key::INHERIT).is_some() {
            // Note that this doesn't catch all missing error variants, but it's the best
            // that can be done without sourcing the ebuild.
            return Err(invalid("missing eclass checksum"));
        }

        Ok(())
    }
}

impl Md5DictEntry {
    /// Serialize a cache entry to raw bytes for writing to a file.
    fn to_bytes(&self) -> Vec<u8> {
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

impl FromStr for Md5DictEntry {
    type Err = Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let mut data = IndexMap::new();
        for line in s.lines() {
            let (k, v) = line.split_once('=').ok_or_else(|| {
                Error::InvalidValue(format!("invalid md5-dict cache line: {line}"))
            })?;
            data.insert(k.parse()?, v.to_string());
        }

        Ok(Self(data))
    }
}

/// Serialize a metadata field to its md5-dict cache mapping, returning None for empty fields.
fn serialize(meta: &Metadata, key: &Key) -> Option<(Md5DictKey, String)> {
    let value = match key {
        Key::CHKSUM => meta.chksum.to_string(),
        Key::DESCRIPTION => meta.description.to_string(),
        Key::SLOT => meta.slot.to_string(),
        Key::BDEPEND => meta.bdepend.to_string(),
        Key::DEPEND => meta.depend.to_string(),
        Key::IDEPEND => meta.idepend.to_string(),
        Key::PDEPEND => meta.pdepend.to_string(),
        Key::RDEPEND => meta.rdepend.to_string(),
        Key::LICENSE => meta.license.to_string(),
        Key::PROPERTIES => meta.properties.to_string(),
        Key::REQUIRED_USE => meta.required_use.to_string(),
        Key::RESTRICT => meta.restrict.to_string(),
        Key::SRC_URI => meta.src_uri.to_string(),
        Key::HOMEPAGE => meta.homepage.iter().join(" "),
        Key::DEFINED_PHASES => {
            // PMS specifies if no phase functions are defined, a single hyphen is used.
            if meta.defined_phases.is_empty() {
                "-".to_string()
            } else {
                meta.defined_phases.iter().map(|p| p.name()).join(" ")
            }
        }
        Key::KEYWORDS => meta.keywords.iter().join(" "),
        Key::IUSE => meta.iuse.iter().join(" "),
        Key::INHERIT => meta.inherit.iter().join(" "),
        Key::INHERITED => meta
            .inherited
            .iter()
            .flat_map(|e| [e.name(), e.chksum()])
            .join("\t"),
        Key::EAPI => meta.eapi.to_string(),
    };

    if value.is_empty() {
        None
    } else {
        Some((Md5DictKey(*key), value))
    }
}

impl From<&Metadata> for Md5DictEntry {
    fn from(meta: &Metadata) -> Self {
        meta.eapi
            .metadata_keys()
            .iter()
            .filter_map(|key| serialize(meta, key))
            .collect()
    }
}

/// The md5-dict metadata cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Md5Dict {
    path: Utf8PathBuf,
}

impl Md5Dict {
    /// Load a metadata cache from the default repo location.
    pub(super) fn from_repo<P: AsRef<Utf8Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().join("metadata/md5-cache"),
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

    fn chksum<S: AsRef<[u8]>>(&self, data: S) -> String {
        digest::<md5::Md5>(data.as_ref())
    }

    fn format(&self) -> CacheFormat {
        CacheFormat::Md5Dict
    }

    fn path(&self) -> &Utf8Path {
        &self.path
    }

    fn get(&self, pkg: &EbuildRawPkg) -> Option<crate::Result<Self::Entry>> {
        let path = self.path.join(pkg.cpv().to_string());
        match fs::read_to_string(&path) {
            Ok(data) => Some(
                data.parse::<Self::Entry>()
                    .and_then(|entry| entry.verify(pkg).and(Ok(entry))),
            ),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => Some(Err(Error::IO(format!("failed loading metadata: {path}: {e}")))),
        }
    }

    fn update(&self, pkg: &EbuildRawPkg, meta: &Metadata) -> crate::Result<()> {
        // convert metadata to the cache entry format
        let entry = Self::Entry::from(meta);
        // atomically create cache file
        let path = self.path.join(pkg.cpv().category()).join(pkg.cpv().pf());
        atomic_write_file(path, entry.to_bytes())
    }

    fn remove(&self, _repo: &EbuildRepo) -> crate::Result<()> {
        let path = &self.path;
        fs::remove_dir_all(path)
            .map_err(|e| Error::IO(format!("failed removing metadata cache: {path}: {e}")))
    }

    fn remove_entry(&self, cpv: &Cpv) -> crate::Result<()> {
        let path = self.path.join(cpv.category()).join(cpv.pf());
        match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                Err(Error::IO(format!("failed removing cache entry: {cpv}: {e}")))
            }
            _ => {
                // remove empty parent directory
                let _ = fs::remove_dir(path.parent().unwrap());
                Ok(())
            }
        }
    }

    fn clean<C: for<'a> Contains<&'a Cpv> + Sync>(&self, collection: C) -> crate::Result<()> {
        // TODO: replace with parallelized cache iterator
        let entries: Vec<_> = WalkDir::new(self.path())
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .collect();

        // remove invalid file and parent directory if empty
        let remove_file = |path: &Utf8Path| -> crate::Result<()> {
            fs::remove_file(path).map_err(|e| {
                Error::IO(format!("failed removing old cache entry: {path}: {e}"))
            })?;

            let dir = path.parent().unwrap();
            if let Err(e) = fs::remove_dir(dir) {
                if e.kind() != io::ErrorKind::DirectoryNotEmpty {
                    return Err(Error::IO(format!("failed removing cache dir: {dir}: {e}")));
                }
            }

            Ok(())
        };

        // Remove outdated, invalid, and unrelated files as well as their parent
        // directories if empty.
        entries
            .into_par_iter()
            .filter_map(Result::ok)
            .filter(is_file)
            .filter_map(|e| Utf8PathBuf::from_path_buf(e.into_path()).ok())
            .try_for_each(|path| -> crate::Result<()> {
                // convert to relative path
                let relpath = path.strip_prefix(self.path()).expect("invalid cache path");

                // determine if a cache file is valid, relating to an existing pkg
                let valid = Cpv::try_new(relpath)
                    .ok()
                    .map(|cpv| collection.contains(&cpv))
                    .unwrap_or_default();

                if !valid {
                    remove_file(&path)?;
                }

                Ok(())
            })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::test::*;

    use super::*;

    #[test]
    fn deserialize() {
        // valid
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        for pkg in repo.iter_raw() {
            let pkg = pkg.unwrap();
            let r = pkg.metadata(false);
            assert!(r.is_ok(), "{pkg}: failed converting to metadata: {}", r.unwrap_err());
        }

        // invalid
        let data = test_data();
        let repo = data.ebuild_repo("metadata-invalid").unwrap();
        for pkg in repo.iter_raw() {
            let pkg = pkg.unwrap();
            let err = fs::read_to_string(pkg.path().parent().unwrap().join("error")).unwrap();
            let err = err.trim();
            assert_err_re!(pkg.metadata(false), format!("^{pkg}: invalid metadata: {err}$"));
        }
    }

    #[test]
    fn update_and_get() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let repo_cache = Md5Dict::from_repo(repo);
        let dir = tempdir().unwrap();
        let new_cache = Md5Dict::from_path(Utf8Path::from_path(dir.path()).unwrap());
        for pkg in repo.iter_raw() {
            let pkg = pkg.unwrap();
            let meta = pkg.metadata(false).unwrap();
            new_cache.update(&pkg, &meta).unwrap();
            let new_entry = new_cache.get(&pkg).unwrap().unwrap();
            let old_entry = repo_cache.get(&pkg).unwrap().unwrap();
            assert_eq!(new_entry, old_entry);
            let new_meta = new_entry.to_metadata(&pkg).unwrap();
            let old_meta = old_entry.to_metadata(&pkg).unwrap();
            assert_eq!(new_meta, old_meta);
        }
    }

    #[test]
    fn invalid_cache_entry() {
        let data = indoc::indoc! {"
            DEFINED_PHASES=-
            DESCRIPTION=ebuild with no subslot
            EAPI=8
            SLOT=1
            _md5_=e9b0c6b982420006ac4ca3ab1195a563
            invalid data
        "};

        let r = Md5DictEntry::from_str(data);
        assert_err_re!(r, format!("^invalid md5-dict cache line: invalid data$"));

        let data = indoc::indoc! {"
            DEFINED_PHASES=-
            DESCRIPTION=ebuild with no subslot
            EAPI=8
            SLOT=1
            _md5_=e9b0c6b982420006ac4ca3ab1195a563
            INVALID=data
        "};

        let r = Md5DictEntry::from_str(data);
        assert_err_re!(r, format!("^invalid md5-dict cache key: INVALID$"));
    }
}
