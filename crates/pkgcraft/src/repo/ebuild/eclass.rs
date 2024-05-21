use std::{fmt, fs};

use camino::{Utf8Path, Utf8PathBuf};
use scallop::{source, ExecStatus};

use crate::dep::parse;
use crate::traits::SourceBash;
use crate::Error;

use super::cache::{Cache, MetadataCache};

/// An eclass in an ebuild repository.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Eclass {
    name: String,
    path: Utf8PathBuf,
    chksum: String,
}

impl Eclass {
    /// Create a new eclass.
    pub(crate) fn try_new(path: &Utf8Path, cache: &MetadataCache) -> crate::Result<Self> {
        if let (Some(name), Some("eclass")) = (path.file_stem(), path.extension()) {
            let data = fs::read(path)
                .map_err(|e| Error::IO(format!("failed reading eclass: {path}: {e}")))?;

            Ok(Self {
                name: parse::eclass_name(name)?.to_string(),
                path: path.to_path_buf(),
                chksum: cache.chksum(data),
            })
        } else {
            Err(Error::InvalidValue(format!("invalid eclass: {path}")))
        }
    }

    /// Return the name of the eclass.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the full path of the eclass.
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Return the MD5 checksum of the eclass.
    pub(crate) fn chksum(&self) -> &str {
        &self.chksum
    }
}

impl fmt::Display for Eclass {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl SourceBash for Eclass {
    fn source_bash(&self) -> scallop::Result<ExecStatus> {
        source::file(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use crate::repo::Repository;
    use crate::test::TEST_DATA;

    use super::*;

    #[test]
    fn try_new() {
        let repo = TEST_DATA.ebuild_repo("metadata").unwrap();
        let cache = repo.metadata.cache();

        // nonexistent path
        assert!(Eclass::try_new(&repo.path().join("eclass/nonexistent.eclass"), cache).is_err());

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
