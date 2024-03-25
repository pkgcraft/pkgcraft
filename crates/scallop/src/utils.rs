// TODO: drop this once stable rust supports `unix_sigpipe`,
// see https://github.com/rust-lang/rust/issues/97889.
//
/// Reset SIGPIPE to the default behavior.
pub fn reset_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}
