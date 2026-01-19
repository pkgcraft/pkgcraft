use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};
use std::{fmt, fs};

use camino::{Utf8Path, Utf8PathBuf};
use scallop::{ExecStatus, source};

use crate::dep::parse;
use crate::traits::SourceBash;
use crate::{Error, bash};

use super::cache::{Cache, MetadataCache};

/// An eclass in an ebuild repository.
#[derive(Debug)]
struct InternalEclass {
    name: String,
    path: Utf8PathBuf,
    data: Arc<String>,
    chksum: String,
    tree: OnceLock<bash::Tree>,
}

#[derive(Debug, Clone)]
pub struct Eclass(Arc<InternalEclass>);

impl Eclass {
    /// Create a new eclass.
    pub(crate) fn try_new(path: &Utf8Path, cache: &MetadataCache) -> crate::Result<Self> {
        if let (Some(name), Some("eclass")) = (path.file_stem(), path.extension()) {
            let data = fs::read_to_string(path)
                .map_err(|e| Error::IO(format!("failed reading eclass: {path}: {e}")))?;

            let chksum = cache.chksum(&data);
            Ok(Self(Arc::new(InternalEclass {
                name: parse::eclass_name(name)?.to_string(),
                path: path.to_path_buf(),
                data: Arc::new(data),
                chksum,
                tree: Default::default(),
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

    /// Return the eclass file content.
    pub fn data(&self) -> &str {
        &self.0.data
    }

    /// Return the MD5 checksum of the eclass.
    pub(crate) fn chksum(&self) -> &str {
        &self.0.chksum
    }

    /// Return the bash parse tree for the eclass.
    pub fn tree(&self) -> &bash::Tree {
        self.0
            .tree
            .get_or_init(|| bash::Tree::new(self.0.data.clone()))
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
        source::string(self.data()).map_err(|e| {
            let name = &self.0.name;
            scallop::Error::Base(format!("failed loading eclass: {name}: {e}"))
        })
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
        let path_a = repo.path().join("eclass/a.eclass");
        let eclass_a = Eclass::try_new(&path_a, cache).unwrap();
        assert_eq!(eclass_a.path(), path_a);
        assert_eq!(eclass_a.name(), "a");
        assert!(!eclass_a.chksum().is_empty());
        assert!(eclass_a == eclass_a);
        let path_b = repo.path().join("eclass/b.eclass");
        let eclass_b = Eclass::try_new(&path_b, cache).unwrap();
        assert!(eclass_a < eclass_b);
    }
}
