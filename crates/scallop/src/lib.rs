#![warn(unreachable_pub)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod bash;
pub mod builtins;
pub mod command;
pub mod error;
pub mod functions;
mod macros;
pub mod pool;
pub mod shell;
pub(crate) mod shm;
pub mod source;
mod test;
pub mod traits;
pub mod utils;
pub mod variables;

pub use self::error::{Error, Result};
