use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

mod ebuild;
mod fake;

type VersionCache = HashMap<String, HashSet<String>>;
type PkgCache = HashMap<String, VersionCache>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Repository {
    Ebuild(ebuild::Repo),
    Fake(fake::Repo),
}

impl fmt::Display for Repository {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Repository::Ebuild(repo) => write!(f, "{}", repo),
            Repository::Fake(repo) => write!(f, "{}", repo),
        }
    }
}

impl Repository {
    pub fn from_path<S: AsRef<str>>(id: S, path: S) -> Result<Repository> {
        let id = id.as_ref();
        let path = path.as_ref();

        if let Ok(repo) = ebuild::Repo::from_path(id, path) {
            return Ok(Repository::Ebuild(repo));
        }

        if let Ok(repo) = fake::Repo::from_path(id, path) {
            return Ok(Repository::Fake(repo));
        }

        Err(Error::ConfigError(format!("{:?} repo at {:?}: unknown or invalid format", id, path)))
    }

    pub fn from_format<S: AsRef<str>>(id: S, path: S, format: S) -> Result<Repository> {
        let id = id.as_ref();
        let path = path.as_ref();
        let format = format.as_ref();

        match format {
            "ebuild" => Ok(Repository::Ebuild(ebuild::Repo::from_path(id, path)?)),
            "fake" => Ok(Repository::Fake(fake::Repo::from_path(id, path)?)),
            _ => {
                let err = format!("{:?} repo: unknown format: {:?}", id, format);
                Err(Error::ConfigError(err))
            },
        }
    }
}

pub trait Repo: fmt::Debug + fmt::Display + Sized {
    // TODO: convert to `impl Iterator` return type once supported within traits
    // https://github.com/rust-lang/rfcs/blob/master/text/1522-conservative-impl-trait.md
    fn categories(&self) -> Box<dyn Iterator<Item = &String> + '_>;
    fn packages<S: AsRef<str>>(&self, cat: S) -> Box<dyn Iterator<Item = &String> + '_>;
    fn versions<S: AsRef<str>>(&self, cat: S, pkg: S) -> Box<dyn Iterator<Item = &String> + '_>;
    fn from_path<S: AsRef<str>>(id: S, path: S) -> Result<Self>;
}
