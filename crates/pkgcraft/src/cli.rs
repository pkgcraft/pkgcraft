mod maybe_stdin;
pub use maybe_stdin::*;
mod targets;
pub use targets::*;
mod tristate;
pub use tristate::*;

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

/// Return true if a given output stream should enable color support, otherwise false.
#[macro_export]
macro_rules! colorize {
    ($stream:expr) => {
        !matches!(anstream::AutoStream::choice($stream), anstream::ColorChoice::Never)
    };
}
pub use colorize;

// TODO: drop this once stable rust supports `unix_sigpipe`,
// see https://github.com/rust-lang/rust/issues/97889.
//
/// Reset SIGPIPE to the default behavior.
pub fn reset_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}
