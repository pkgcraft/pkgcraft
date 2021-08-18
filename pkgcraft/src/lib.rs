#![warn(unreachable_pub)]

pub mod atom;
pub mod config;
pub mod depspec;
pub mod eapi;
mod error;
mod macros;
mod repo;
mod sync;
mod utils;

pub use self::error::{Error, Result};
