use std::path::PathBuf;

use thiserror::Error;

use crate::atom;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Eapi(String),
    #[error("atom parse error")]
    ParseAtom(#[from] atom::ParseError),
    #[error("config error: {0}")]
    Config(String),
    #[error("invalid repo: {path:?}: {error}")]
    RepoInvalid { path: PathBuf, error: String },
    #[error("{0}")]
    RepoInit(String),
    #[error("failed syncing repo: {0}")]
    RepoSync(String),
}
