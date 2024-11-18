use std::borrow::Borrow;
use std::str::FromStr;
use std::{fmt, fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use itertools::Itertools;
use rayon::prelude::*;
use tracing::warn;
use walkdir::WalkDir;

use crate::dep::{Cpv, DependencySet, Slot};
use crate::eapi::Eapi;
use crate::files::{atomic_write_file, is_file};
use crate::pkg::ebuild::metadata::{Key, Metadata};
use crate::pkg::ebuild::{iuse::Iuse, keyword::Keyword, EbuildRawPkg};
use crate::pkg::{Package, RepoPackage};
use crate::repo::ebuild::{EbuildRepo, Eclass};
use crate::shell::phase::Phase;
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
            s => s
                .parse()
                .map_err(|_| Error::InvalidValue(format!("invalid md5-dict cache key: {s}")))?,
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
#[derive(Debug, Default)]
pub struct Md5DictEntry(IndexMap<Md5DictKey, String>);

/// Deserialize a cache entry value to its Metadata field value.
fn deserialize(
    meta: &mut Metadata,
    eapi: &'static Eapi,
    repo: &EbuildRepo,
    key: &Key,
    val: &str,
) -> crate::Result<()> {
    // return the Eclass for a given identifier if it exists
    let eclass = |name: &str| -> crate::Result<Eclass> {
        repo.eclasses()
            .get(name)
            .cloned()
            .ok_or_else(|| Error::InvalidValue(format!("nonexistent eclass: {name}")))
    };

    // return the Keyword for a given identifier if it exists
    let keyword = |s: &str| -> crate::Result<Keyword> {
        let keyword = Keyword::try_new(s)?;
        let arch = keyword.arch();
        if arch != "*" && !repo.arches().contains(arch) {
            Err(Error::InvalidValue(format!("nonexistent arch: {arch}")))
        } else {
            Ok(keyword)
        }
    };

    // return the Phase for a given name if it exists
    let phase = |name: &str| -> crate::Result<Phase> {
        eapi.phases()
            .get(name)
            .copied()
            .ok_or_else(|| Error::InvalidValue(format!("nonexistent phase: {name}")))
    };

    use Key::*;
    match key {
        CHKSUM => meta.chksum = val.to_string(),
        DESCRIPTION => meta.description = val.to_string(),
        SLOT => meta.slot = Slot::try_new(val)?,
        BDEPEND => meta.bdepend = DependencySet::package(val, eapi)?,
        DEPEND => meta.depend = DependencySet::package(val, eapi)?,
        IDEPEND => meta.idepend = DependencySet::package(val, eapi)?,
        PDEPEND => meta.pdepend = DependencySet::package(val, eapi)?,
        RDEPEND => meta.rdepend = DependencySet::package(val, eapi)?,
        LICENSE => {
            meta.license = DependencySet::license(val)?;
            for l in meta.license.iter_flatten() {
                if !repo.licenses().contains(l) {
                    return Err(Error::InvalidValue(format!("nonexistent license: {l}")));
                }
            }
        }
        PROPERTIES => meta.properties = DependencySet::properties(val)?,
        REQUIRED_USE => meta.required_use = DependencySet::required_use(val)?,
        RESTRICT => meta.restrict = DependencySet::restrict(val)?,
        SRC_URI => meta.src_uri = DependencySet::src_uri(val)?,
        HOMEPAGE => meta.homepage = val.split_whitespace().map(String::from).collect(),
        DEFINED_PHASES => {
            // PMS specifies if no phase functions are defined, a single hyphen is used.
            if val != "-" {
                meta.defined_phases = val.split_whitespace().map(phase).try_collect()?
            }
        }
        KEYWORDS => meta.keywords = val.split_whitespace().map(keyword).try_collect()?,
        IUSE => meta.iuse = val.split_whitespace().map(Iuse::try_new).try_collect()?,
        INHERIT => meta.inherit = val.split_whitespace().map(eclass).try_collect()?,
        INHERITED => {
            meta.inherited = val
                .split_whitespace()
                .tuples()
                .map(|(name, _chksum)| eclass(name))
                .try_collect()?
        }
        EAPI => {
            let sourced: &Eapi = val.try_into()?;
            if sourced != eapi {
                return Err(Error::InvalidValue(format!(
                    "mismatched sourced and parsed EAPIs: {sourced} != {eapi}"
                )));
            }
            meta.eapi = eapi;
        }
    }

    Ok(())
}

impl CacheEntry for Md5DictEntry {
    fn to_metadata(&self, pkg: &EbuildRawPkg) -> crate::Result<Metadata> {
        let mut meta = Metadata::default();

        for key in pkg.eapi().mandatory_keys() {
            if !self.0.contains_key(key) {
                return Err(Error::InvalidValue(format!("missing required value: {key}")));
            }
        }

        for key in pkg.eapi().metadata_keys() {
            if let Some(val) = self.0.get(key) {
                deserialize(&mut meta, pkg.eapi(), &pkg.repo(), key, val)?;
            }
        }

        Ok(meta)
    }

    fn verify(&self, pkg: &EbuildRawPkg) -> crate::Result<()> {
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
            let repo = pkg.repo();
            for (name, chksum) in val.split_whitespace().tuples() {
                let Some(eclass) = repo.eclasses().get(name) else {
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
    use Key::*;
    let value = match key {
        CHKSUM => meta.chksum.to_string(),
        DESCRIPTION => meta.description.to_string(),
        SLOT => meta.slot.to_string(),
        BDEPEND => meta.bdepend.to_string(),
        DEPEND => meta.depend.to_string(),
        IDEPEND => meta.idepend.to_string(),
        PDEPEND => meta.pdepend.to_string(),
        RDEPEND => meta.rdepend.to_string(),
        LICENSE => meta.license.to_string(),
        PROPERTIES => meta.properties.to_string(),
        REQUIRED_USE => meta.required_use.to_string(),
        RESTRICT => meta.restrict.to_string(),
        SRC_URI => meta.src_uri.to_string(),
        HOMEPAGE => meta.homepage.iter().join(" "),
        DEFINED_PHASES => {
            // PMS specifies if no phase functions are defined, a single hyphen is used.
            if meta.defined_phases.is_empty() {
                "-".to_string()
            } else {
                meta.defined_phases.iter().map(|p| p.name()).join(" ")
            }
        }
        KEYWORDS => meta.keywords.iter().join(" "),
        IUSE => meta.iuse.iter().join(" "),
        INHERIT => meta.inherit.iter().join(" "),
        INHERITED => meta
            .inherited
            .iter()
            .flat_map(|e| [e.name(), e.chksum()])
            .join("\t"),
        EAPI => meta.eapi.to_string(),
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
#[derive(Debug)]
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

    fn get(&self, pkg: &EbuildRawPkg) -> crate::Result<Self::Entry> {
        let path = self.path.join(pkg.cpv().to_string());
        let data = fs::read_to_string(&path).map_err(|e| {
            if e.kind() != io::ErrorKind::NotFound {
                warn!("error loading ebuild metadata: {path}: {e}");
            }
            Error::IO(format!("failed loading ebuild metadata: {path}: {e}"))
        })?;

        let meta = data.parse::<Self::Entry>()?;
        meta.verify(pkg)?;
        Ok(meta)
    }

    fn update(&self, pkg: &EbuildRawPkg, meta: &Metadata) -> crate::Result<()> {
        // determine cache entry directory
        let path = self.path.join(pkg.cpv().category());

        // convert metadata to the cache entry format
        let entry = Self::Entry::from(meta);

        // atomically create cache file
        atomic_write_file(&path, &pkg.cpv().pf(), entry.to_bytes())
    }

    fn remove(&self, _repo: &EbuildRepo) -> crate::Result<()> {
        let path = &self.path;
        fs::remove_dir_all(path)
            .map_err(|e| Error::IO(format!("failed removing metadata cache: {path}: {e}")))
    }

    fn clean<C: for<'a> Contains<&'a Cpv> + Sync>(&self, collection: C) -> crate::Result<()> {
        // TODO: replace with parallelized cache iterator
        let entries: Vec<_> = WalkDir::new(self.path())
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .collect();

        // Remove outdated, invalid, and unrelated files as well as their parent directories if
        // empty while ignoring I/O errors.
        entries
            .into_par_iter()
            .filter_map(|e| e.ok())
            .filter(is_file)
            .for_each(|e| {
                if let Some(path) = Utf8Path::from_path(e.path()) {
                    if let Ok(relpath) = path.strip_prefix(self.path()) {
                        // determine if a cache file is valid, relating to an existing pkg
                        let valid = Cpv::try_new(relpath.as_str())
                            .ok()
                            .map(|cpv| collection.contains(&cpv))
                            .unwrap_or_default();
                        if !valid {
                            fs::remove_file(path).ok();
                            fs::remove_dir(path.parent().unwrap()).ok();
                        }
                    }
                }
            });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test::TEST_DATA;

    use super::*;

    #[test]
    fn load_and_convert() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        let cache = CacheFormat::Md5Dict.from_repo(repo);
        for pkg in repo.iter_raw() {
            let r = cache.get(&pkg);
            assert!(r.is_ok(), "{pkg}: failed loading cache entry: {}", r.unwrap_err());
            let r = r.unwrap().to_metadata(&pkg);
            assert!(r.is_ok(), "{pkg}: failed converting to metadata: {}", r.unwrap_err());
        }
    }
}
