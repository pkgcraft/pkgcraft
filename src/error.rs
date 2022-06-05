use camino::Utf8PathBuf;

use crate::peg;

/// A `Result` alias where the `Err` case is `pkgcraft::Error`.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Eapi(String),
    #[error("{0}")]
    PegParse(peg::Error),
    #[error("config error: {0}")]
    Config(String),
    #[error("{0}")]
    InvalidValue(String),
    #[error("invalid repo: {path}: {error}")]
    InvalidRepo { path: Utf8PathBuf, error: String },
    #[error("{0}")]
    IO(String),
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
        let s = e.to_string();
        match scallop::builtins::running_builtin() {
            Some(_) => scallop::Error::Builtin(s),
            None => scallop::Error::Base(s),
        }
    }
}
