#![warn(unreachable_pub)]

use std::sync::atomic::AtomicBool;

pub(crate) mod archive;
pub(crate) mod command;
pub mod config;
pub mod dep;
pub mod eapi;
mod error;
pub(crate) mod files;
pub mod macros;
pub mod peg;
pub mod pkg;
pub mod repo;
pub mod restrict;
pub mod shell;
mod sync;
pub mod test;
pub mod traits;
pub mod types;
pub mod utils;

pub use self::error::{Error, Result};

/// Controls if lazy fields are collapsed on initialized for process parallelization efficiency.
static COLLAPSE_LAZY_FIELDS: AtomicBool = AtomicBool::new(false);
