mod maybe_stdin;
pub use maybe_stdin::*;
mod targets;
pub use targets::*;

/// Return true if a given file descriptor is a terminal/tty, otherwise false.
///
/// Allows overriding the return value for testing purposes.
#[macro_export]
macro_rules! is_terminal {
    ($fd:expr) => {
        std::io::IsTerminal::is_terminal($fd)
            || (cfg!(feature = "test") && std::env::var("PKGCRAFT_IS_TERMINAL").is_ok())
    };
}
pub use is_terminal;
