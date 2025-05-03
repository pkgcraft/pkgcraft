pub(crate) mod archive;
pub mod bash;
pub mod cli;
pub(crate) mod command;
pub mod config;
pub mod dep;
pub mod eapi;
pub mod error;
pub mod fetch;
pub mod files;
pub(crate) mod io;
pub mod macros;
pub mod pkg;
pub mod repo;
pub mod restrict;
pub mod shell;
mod sync;
#[cfg(any(feature = "test", test))]
pub mod test;
pub mod traits;
pub mod types;
pub mod utils;
pub(crate) mod xml;

pub use self::error::Error;

/// A `Result` alias where the `Err` case is `pkgcraft::Error`.
pub type Result<T> = std::result::Result<T, Error>;
