use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::{fmt, fs};

use camino::{Utf8Path, Utf8PathBuf};
use scallop::{ExecStatus, source};

use crate::Error;
use crate::dep::parse;
use crate::traits::SourceBash;

use super::cache::{Cache, MetadataCache};

/// An eclass in an ebuild repository.
#[derive(Debug)]
struct InternalEclass {
    name: String,
    path: Utf8PathBuf,
    chksum: String,
}

#[derive(Debug, Clone)]
pub struct Eclass(Arc<InternalEclass>);

impl Eclass {
    /// Create a new eclass.
    pub(crate) fn try_new(path: &Utf8Path, cache: &MetadataCache) -> crate::Result<Self> {
        if let (Some(name), Some("eclass")) = (path.file_stem(), path.extension()) {
            let data = fs::read(path)
                .map_err(|e| Error::IO(format!("failed reading eclass: {path}: {e}")))?;

            Ok(Self(Arc::new(InternalEclass {
                name: parse::eclass_name(name)?.to_string(),
                path: path.to_path_buf(),
                chksum: cache.chksum(data),
            })))
        } else {
            Err(Error::InvalidValue(format!("invalid eclass: {path}")))
        }
    }

    /// Return the name of the eclass.
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// Return the full path of the eclass.
    pub fn path(&self) -> &Utf8Path {
        &self.0.path
    }

    /// Return the MD5 checksum of the eclass.
    pub(crate) fn chksum(&self) -> &str {
        &self.0.chksum
    }
}

impl PartialEq for Eclass {
    fn eq(&self, other: &Self) -> bool {
        self.0.name == other.0.name
    }
}

impl Eq for Eclass {}

impl Hash for Eclass {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.name.hash(state);
    }
}

impl Borrow<str> for Eclass {
    fn borrow(&self) -> &str {
        &self.0.name
    }
}

impl Ord for Eclass {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.name.cmp(&other.0.name)
    }
}

impl PartialOrd for Eclass {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Eclass {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.name)
    }
}

impl SourceBash for Eclass {
    fn source_bash(&self) -> scallop::Result<ExecStatus> {
        source::file(&self.0.path)
    }
}

#[cfg(test)]
mod tests {
    use crate::test::test_data;

    use super::*;

    #[test]
    fn try_new() {
        let data = test_data();
        let repo = data.ebuild_repo("metadata").unwrap();
        let cache = repo.metadata().cache();

        // nonexistent path
        assert!(
            Eclass::try_new(&repo.path().join("eclass/nonexistent.eclass"), cache).is_err()
        );

        // non-eclass path
        assert!(Eclass::try_new(&repo.path().join("licenses/l1"), cache).is_err());

        // valid
        let path = repo.path().join("eclass/a.eclass");
        let eclass = Eclass::try_new(&path, cache).unwrap();
        assert_eq!(eclass.path(), path);
        assert_eq!(eclass.name(), "a");
        assert!(!eclass.chksum().is_empty());
    }
}
