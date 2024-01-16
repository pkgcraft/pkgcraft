#![warn(unreachable_pub)]

pub mod check;
pub mod error;
pub mod report;
pub mod reporter;
pub mod runner;
pub mod scanner;
pub mod scope;
pub mod source;

pub use self::error::Error;

/// A `Result` alias where the `Err` case is `pkgcraft::Error`.
pub type Result<T> = std::result::Result<T, Error>;
