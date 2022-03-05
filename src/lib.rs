#![warn(unreachable_pub)]

pub(crate) mod archive;
pub mod atom;
pub(crate) mod command;
pub mod config;
pub mod depspec;
pub mod eapi;
mod error;
mod macros;
pub mod peg;
pub mod pkgsh;
mod repo;
mod sync;
#[cfg(test)]
pub(crate) mod test;
mod types;
mod utils;

pub use self::error::{Error, Result};
