mod error;
pub mod git;
pub mod proto;
pub mod service;

pub use self::error::Error;
pub use self::proto::pkgcruft_client::PkgcruftClient as Client;

/// A `Result` alias where the `Err` case is `pkgcruft_git::Error`.
pub type Result<T> = std::result::Result<T, Error>;
