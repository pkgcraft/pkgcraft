#![warn(unreachable_pub)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod bash;
pub mod builtins;
pub mod command;
pub mod error;
pub mod functions;
pub(crate) mod scallop;
pub mod shell;
pub mod source;
pub mod traits;
pub mod variables;

pub use self::error::{Error, Result};

#[cfg(test)]
mod tests {
    #[cfg(feature = "plugin")]
    compile_error!("The feature \"plugin\" must be disabled for testing.");
}
