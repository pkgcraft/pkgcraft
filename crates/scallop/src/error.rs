use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use crate::{ExecStatus, bash};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(
    Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, thiserror::Error,
)]
pub enum Error {
    #[error("{0}")]
    Bail(String),
    #[error("{0}")]
    Base(String),
    #[error("{0}")]
    IO(String),
    #[error("failed: {0}")]
    Status(i32),
}

// grcov-excl-start: most I/O errors are hard to emulate
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(format!("{e}: {}", e.kind()))
    }
}
// grcov-excl-stop

static CALL_LEVEL: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static ERRORS: RefCell<HashMap<usize, Error>> = RefCell::new(Default::default());
}

/// Reset the cached error support.
pub(crate) fn reset() {
    ERRORS.with(|errors| errors.borrow_mut().clear());
    CALL_LEVEL.store(0, Ordering::Relaxed);
}

/// Run a function encompassing bash C calls that may spawn errors, returning the most recent if it
/// exists. Otherwise, the error status is pulled from the integer-based function result, zero for
/// success and nonzero for failure.
pub(crate) fn ok_or_error<F: FnOnce() -> Result<ExecStatus>>(func: F) -> Result<ExecStatus> {
    CALL_LEVEL.fetch_add(1, Ordering::Relaxed);
    let result = func();
    crate::shell::raise_shm_error();
    let level = CALL_LEVEL.fetch_sub(1, Ordering::Relaxed);
    match ERRORS.with(|errors| errors.borrow_mut().remove(&level)) {
        None => result,
        Some(e) => Err(e),
    }
}

/// Wrapper to convert bash errors into native errors.
pub(crate) fn bash_error(msg: *mut c_char, status: u8) {
    let raw_msg = unsafe { CStr::from_ptr(msg) };
    // panic on error messages with invalid UTF-8
    let msg = raw_msg
        .to_str()
        .unwrap_or_else(|e| panic!("invalid error message: {e}"));

    // Strip the shell name prefix that bash adds -- can't easily do this in bash since the same
    // functionality is used for shell script names which shouldn't be stripped.
    let msg = msg.strip_prefix("scallop: ").unwrap_or(msg);

    // panic on empty messages
    assert!(!msg.is_empty());

    let level = CALL_LEVEL.load(Ordering::Relaxed);
    ERRORS.with(|errors| {
        let e = if status == bash::EX_LONGJMP as u8 {
            Error::Bail(msg.to_string())
        } else {
            Error::Base(msg.to_string())
        };
        errors.borrow_mut().insert(level, e);
    })
}

// grcov-excl-start: bash only uses warnings for internal issues
/// Output given message as warning level log message.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn bash_warning_log(msg: *mut c_char) {
    let msg = unsafe { CStr::from_ptr(msg).to_string_lossy() };
    tracing::warn!("{msg}");
}
// grcov-excl-stop

/// Wrapper to write errors and warning to stderr for interactive mode.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn stderr_output(msg: *mut c_char) {
    let msg = unsafe { CStr::from_ptr(msg).to_string_lossy() };
    eprintln!("{msg}");
}
