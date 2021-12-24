#![warn(unreachable_pub)]

pub mod atom;
pub mod bash;
pub mod config;
pub mod depspec;
pub mod eapi;
mod error;
mod macros;
pub mod peg;
mod repo;
mod sync;
pub mod utils;

pub use self::error::{Error, Result};
