use std::io;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Pkgcraft(String),
    #[error("{0}")]
    InvalidValue(String),
    #[error("no matches found")]
    NoMatches,
    #[error("skipping remaining checks due to failure")]
    SkipRemainingChecks,
    #[error("{0}")]
    IO(String),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(format!("{e}: {}", e.kind()))
    }
}

impl From<pkgcraft::Error> for Error {
    fn from(e: pkgcraft::Error) -> Self {
        Error::Pkgcraft(e.to_string())
    }
}
