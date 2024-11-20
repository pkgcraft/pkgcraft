use std::convert::Infallible;
use std::io;

use serde::{Deserialize, Serialize};

use crate::pkg::Package;
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
    #[error("invalid {kind} repo: {id}: {err}")]
    NotARepo {
        kind: RepoFormat,
        id: String,
        err: String,
    },
    #[error("{kind} repo can't be manually loaded: {id}")]
    LoadRepo { kind: RepoFormat, id: String },
    #[error("invalid pkg: {id}: {err}")]
    InvalidPkg { id: String, err: String },
    #[error("{id}: {err}")]
    Pkg { id: String, err: String },
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
    #[error("timed out: {0}")]
    Timeout(String),
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

pub(crate) trait PackageError: Package {
    fn invalid_pkg_err<E: std::error::Error>(&self, err: E) -> Error {
        Error::InvalidPkg {
            id: self.to_string(),
            err: err.to_string(),
        }
    }

    fn pkg_err<E: std::error::Error>(&self, err: E) -> Error {
        Error::Pkg {
            id: self.to_string(),
            err: err.to_string(),
        }
    }
}
