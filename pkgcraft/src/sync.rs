use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::error::{Error, Result};

mod git;

#[derive(Debug, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
pub enum Syncer {
    Git(git::Repo),
    Noop,
}

impl fmt::Display for Syncer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Syncer::Git(repo) => write!(f, "{}", repo.url),
            Syncer::Noop => write!(f, "\"\""),
        }
    }
}

pub trait Syncable {
    fn url_to_syncer(url: &str) -> Result<Syncer>;
    fn sync<P: AsRef<Path>>(&self, path: P) -> Result<()>;
}

impl Syncer {
    pub fn sync<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        match self {
            Syncer::Git(repo) => repo.sync(path),
            Syncer::Noop => Ok(()),
        }
    }
}

impl FromStr for Syncer {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let prioritized_syncers = [git::Repo::url_to_syncer];

        let mut syncer = Syncer::Noop;
        for func in prioritized_syncers.iter() {
            if let Ok(sync) = func(s) {
                syncer = sync;
                break;
            }
        }

        Ok(syncer)
    }
}
