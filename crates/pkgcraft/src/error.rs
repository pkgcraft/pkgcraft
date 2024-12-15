use std::convert::Infallible;
use std::io;

use serde::{Deserialize, Serialize};

use crate::dep::Cpv;
use crate::pkg::{Package, RepoPackage};
use crate::repo::RepoFormat;

mod peg;

pub(crate) use self::peg::peg_error;

#[derive(Debug, Clone, Deserialize, Serialize, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    PegParse(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("config error: {0}")]
    ConfigMissing(String),
    #[error("{0}")]
    InvalidValue(String),
    #[error("invalid repo: {id}: {err}")]
    InvalidRepo { id: String, err: String },
    #[error("nonexistent masters: {}", repos.join(", "))]
    NonexistentRepoMasters { repos: Vec<String> },
    #[error("invalid {kind} repo: {id}: {err}")]
    NotARepo {
        kind: RepoFormat,
        id: String,
        err: String,
    },
    #[error("{kind} repo can't be manually loaded: {id}")]
    LoadRepo { kind: RepoFormat, id: String },
    #[error("invalid pkg: {cpv}::{repo}: {err}")]
    InvalidPkg {
        cpv: Box<Cpv>,
        repo: String,
        err: String,
    },
    #[error("{cpv}::{repo}: {err}")]
    Pkg {
        cpv: Box<Cpv>,
        repo: String,
        err: String,
    },
    #[error("{0}")]
    IO(String),
    #[error("{0}")]
    Overflow(String),
    #[error("{0}")]
    Pkgsh(#[from] scallop::Error),
    #[error("{0}")]
    RepoInit(String),
    #[error("failed syncing repo: {0}")]
    RepoSync(String),
    #[error("fetch failed: {uri}: {reason}")]
    FetchFailed { uri: String, reason: String },
}

impl From<Error> for scallop::Error {
    fn from(e: Error) -> Self {
        scallop::Error::Base(e.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(format!("{e}: {}", e.kind()))
    }
}

// Stub for infallible From<T> conversion types.
// TODO: This should be able to be dropped when upstream stabilizes:
// https://github.com/rust-lang/rust/issues/64715.
impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

pub(crate) trait PackageError: Package + RepoPackage {
    fn invalid_pkg_err<E: std::error::Error>(&self, err: E) -> Error {
        Error::InvalidPkg {
            cpv: Box::new(self.cpv().clone()),
            repo: self.repo().to_string(),
            err: err.to_string(),
        }
    }

    fn pkg_err<E: std::error::Error>(&self, err: E) -> Error {
        Error::Pkg {
            cpv: Box::new(self.cpv().clone()),
            repo: self.repo().to_string(),
            err: err.to_string(),
        }
    }
}
