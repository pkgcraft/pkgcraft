#![warn(unreachable_pub)]

pub mod atom;
pub mod config;
pub mod depspec;
pub mod eapi;
mod error;
mod macros;
pub mod peg;
pub mod pkgsh;
mod repo;
mod sync;

pub use self::error::{Error, Result};
