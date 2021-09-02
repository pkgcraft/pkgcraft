/// A `Result` alias where the `Err` case is `arcanist::Error`.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed connecting to arcanist: {0}")]
    Connect(String),
    #[error("failed starting arcanist: {0}")]
    Start(String),
}
