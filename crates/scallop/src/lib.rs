#![deny(unsafe_op_in_unsafe_fn)]

pub mod array;
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
pub mod status;
mod test;
pub mod traits;
pub mod variables;

pub use self::error::{Error, Result};
pub use self::status::ExecStatus;
