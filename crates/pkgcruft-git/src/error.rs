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
    Pkgcraft(Box<pkgcraft::Error>),
    #[error("{0}")]
    Pkgcruft(Box<pkgcruft::Error>),
    #[error("{0}")]
    Git(String),
    #[error("{0}")]
    Git2(#[from] git2::Error),
}

impl From<Error> for tonic::Status {
    fn from(e: Error) -> Self {
        tonic::Status::from_error(Box::new(e))
    }
}

impl From<pkgcraft::Error> for Error {
    fn from(e: pkgcraft::Error) -> Self {
        Self::Pkgcraft(Box::new(e))
    }
}

impl From<pkgcruft::Error> for Error {
    fn from(e: pkgcruft::Error) -> Self {
        Self::Pkgcruft(Box::new(e))
    }
}
