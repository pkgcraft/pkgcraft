#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed connecting to pkgcruft-gitd: {0}")]
    Connect(String),
    #[error("failed starting pkgcruft-gitd: {0}")]
    Start(String),
    #[error("invalid push request: {0}")]
    InvalidPushRequest(String),
    #[error("{0}")]
    Pkgcraft(#[from] pkgcraft::Error),
}
