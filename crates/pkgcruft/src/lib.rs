pub mod check;
pub mod error;
pub mod iter;
pub mod report;
pub mod reporter;
mod runner;
pub mod scanner;
pub mod source;
#[cfg(feature = "test")]
pub mod test;
mod utils;

pub use self::error::Error;

/// A `Result` alias where the `Err` case is `pkgcraft::Error`.
pub type Result<T> = std::result::Result<T, Error>;
