#![warn(unreachable_pub)]

pub(crate) mod archive;
pub(crate) mod command;
pub mod config;
pub mod dep;
pub mod eapi;
pub mod error;
pub(crate) mod files;
pub mod macros;
pub mod pkg;
pub mod repo;
pub mod restrict;
pub mod shell;
mod sync;
pub mod test;
pub mod traits;
pub mod types;
pub mod utils;

pub use self::error::Error;

/// A `Result` alias where the `Err` case is `pkgcraft::Error`.
pub type Result<T> = std::result::Result<T, Error>;
