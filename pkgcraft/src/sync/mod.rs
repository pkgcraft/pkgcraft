use std::fmt;
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
    fn sync(&self, path: &str) -> Result<()>;
}

impl Syncer {
    pub fn sync(&self, path: &str) -> Result<()> {
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
