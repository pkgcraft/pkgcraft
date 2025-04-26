use std::convert::Infallible;
use std::io;

use serde::{Deserialize, Serialize};

use crate::dep::{Cpn, Cpv, Uri};
use crate::fetch::Fetchable;
use crate::pkg::{Package, RepoPackage};
use crate::repo::RepoFormat;

mod peg;

pub(crate) use self::peg::peg_error;

#[derive(
    Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, thiserror::Error,
)]
pub enum Error {
    #[error("{0}")]
    PegParse(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("config error: {0}")]
    ConfigMissing(String),
    #[error("invalid fetchable: {0}")]
    InvalidFetchable(String),
    #[error("restricted fetchable: {0}")]
    RestrictedFetchable(Box<Fetchable>),
    #[error("restricted file: {0}")]
    RestrictedFile(Box<Uri>),
    #[error("{0}")]
    InvalidValue(String),
    #[error("invalid repo: {id}: {err}")]
    InvalidRepo { id: String, err: String },
    #[error("nonexistent repo: {0}")]
    NonexistentRepo(String),
    #[error("no matches found: {0}")]
    NoMatches(String),
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
        err: Box<Error>,
    },
    #[error("{cpv}::{repo}: {err}")]
    Pkg {
        cpv: Box<Cpv>,
        repo: String,
        err: Box<Error>,
    },
    #[error("{cpn}::{repo}: {err}")]
    UnversionedPkg {
        cpn: Box<Cpn>,
        repo: String,
        err: Box<Error>,
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
    #[error("fetch failed: {url}: {reason}")]
    FetchFailed { url: String, reason: String },
}

impl Error {
    /// Convert an error into an invalid package error.
    pub fn into_invalid_pkg_err<P>(self, pkg: P) -> Self
    where
        P: Package + RepoPackage,
    {
        Self::InvalidPkg {
            cpv: Box::new(pkg.cpv().clone()),
            repo: pkg.repo().to_string(),
            err: Box::new(self),
        }
    }

    /// Convert an error into a package error.
    pub fn into_pkg_err<P>(self, pkg: P) -> Self
    where
        P: Package + RepoPackage,
    {
        Self::Pkg {
            cpv: Box::new(pkg.cpv().clone()),
            repo: pkg.repo().to_string(),
            err: Box::new(self),
        }
    }

    /// Convert an error into an unversioned package error.
    pub fn into_unversioned_pkg_err<S>(self, cpn: &Cpn, repo: S) -> Self
    where
        S: std::fmt::Display,
    {
        Self::UnversionedPkg {
            cpn: Box::new(cpn.clone()),
            repo: repo.to_string(),
            err: Box::new(self),
        }
    }
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
