use std::path::PathBuf;

use thiserror::Error as ThisError;

use crate::atom;

/// A `Result` alias where the `Err` case is `pkgcraft::Error`.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("{0}")]
    Eapi(String),
    #[error("atom parse error")]
    ParseAtom(#[from] atom::ParseError),
    #[error("config error: {0}")]
    Config(String),
    #[error("{0}")]
    InvalidValue(String),
    #[error("invalid repo: {path:?}: {error}")]
    InvalidRepo { path: PathBuf, error: String },
    #[error("{0}")]
    RepoInit(String),
    #[error("failed syncing repo: {0}")]
    RepoSync(String),
}
