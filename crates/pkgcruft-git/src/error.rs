#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Service(String),
    #[error("invalid push request: {0}")]
    InvalidPushRequest(String),
    #[error("{0}")]
    InvalidValue(String),
    #[error("{0}")]
    IO(String),
    #[error("{0}")]
    Pkgcraft(#[from] pkgcraft::Error),
    #[error("{0}")]
    Pkgcruft(#[from] pkgcruft::Error),
    #[error("{0}")]
    Git(#[from] git2::Error),
}
