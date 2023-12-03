use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::{fmt, fs};

use camino::{Utf8Path, Utf8PathBuf};
use scallop::{source, ExecStatus};

use crate::dep::parse;
use crate::traits::SourceBash;
use crate::utils::digest;
use crate::Error;

#[derive(Debug, Clone)]
pub struct Eclass {
    name: String,
    path: Utf8PathBuf,
    chksum: String,
}

impl Eclass {
    pub(crate) fn new(path: &Utf8Path) -> crate::Result<Self> {
        if let (Some(name), Some("eclass")) = (path.file_stem(), path.extension()) {
            let data = fs::read(path)
                .map_err(|e| Error::IO(format!("failed reading eclass: {path}: {e}")))?;

            Ok(Self {
                name: parse::eclass_name(name)?.to_string(),
                path: path.to_path_buf(),
                chksum: digest::<md5::Md5>(&data),
            })
        } else {
            Err(Error::InvalidValue(format!("invalid eclass: {path}")))
        }
    }

    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    pub fn chksum(&self) -> &str {
        &self.chksum
    }
}

impl fmt::Display for Eclass {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl AsRef<str> for Eclass {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl Eq for Eclass {}

impl PartialEq for Eclass {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Ord for Eclass {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Eclass {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for Eclass {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Borrow<str> for Eclass {
    fn borrow(&self) -> &str {
        &self.name
    }
}

impl SourceBash for Eclass {
    fn source_bash(&self) -> scallop::Result<ExecStatus> {
        source::file(&self.path)
    }
}
