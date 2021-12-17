#![warn(unreachable_pub)]

pub mod atom;
pub mod config;
pub mod depspec;
pub mod eapi;
mod error;
mod macros;
mod peg;
mod repo;
mod sync;
pub mod utils;

#[cfg(feature = "capi")]
mod capi;

pub use self::error::{Error, Result};
