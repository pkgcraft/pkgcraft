use std::path::PathBuf;

use crate::peg;

/// A `Result` alias where the `Err` case is `pkgcraft::Error`.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    PegParse(peg::Error),
    #[error("config error: {0}")]
    Config(String),
    #[error("{0}")]
    InvalidValue(String),
    #[error("invalid repo: {id}: {err}")]
    InvalidRepo { id: String, err: String },
    #[error("invalid pkg: {path}: {err}")]
    InvalidPkg { path: PathBuf, err: String },
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
