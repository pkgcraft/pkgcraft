#![warn(unreachable_pub)]

pub(crate) mod archive;
pub(crate) mod command;
pub mod config;
pub mod dep;
pub mod eapi;
mod error;
pub(crate) mod files;
mod macros;
pub mod peg;
pub mod pkg;
pub mod pkgsh;
pub mod repo;
pub mod restrict;
pub mod set;
mod sync;
pub mod test;
pub mod utils;

pub use self::error::{Error, Result};
