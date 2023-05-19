use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, CStr};
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::builtins::ExecStatus;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Bail(String),
    #[error("{0}")]
    Base(String),
    #[error("{1}")]
    IO(io::ErrorKind, String),
    #[error("{1}")]
    Status(ExecStatus, String),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(e.kind(), e.to_string())
    }
}

static CALL_LEVEL: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static ERRORS: RefCell<HashMap<usize, Error>> = RefCell::new(Default::default());
}

pub(crate) fn reset() {
    CALL_LEVEL.store(0, Ordering::Relaxed);
}

/// Return the most recent error if one exists, otherwise Ok(ExecStatus::Success).
pub(crate) fn ok_or_error<F: FnOnce()>(func: F) -> Result<ExecStatus> {
    CALL_LEVEL.fetch_add(1, Ordering::Relaxed);
    func();
    crate::shell::raise_shm_error();
    let level = CALL_LEVEL.fetch_sub(1, Ordering::Relaxed);
    match ERRORS.with(|errors| errors.borrow_mut().remove(&level)) {
        None => Ok(ExecStatus::Success),
        Some(e) => Err(e),
    }
}

/// Wrapper to convert bash errors into native errors.
#[no_mangle]
pub(crate) extern "C" fn bash_error(msg: *mut c_char) {
    let msg = unsafe { CStr::from_ptr(msg).to_string_lossy() };

    // Strip the shell name prefix that bash adds -- can't easily do this in bash since the same
    // functionality is used for shell script names which shouldn't be stripped.
    let msg = msg.strip_prefix("scallop: ").unwrap_or(&msg);

    if !msg.is_empty() {
        let level = CALL_LEVEL.load(Ordering::Relaxed);
        ERRORS.with(|errors| {
            let err = io::Error::last_os_error();
            // convert bash IO errors into scallop IO errors
            let e = match err.raw_os_error() {
                Some(v) if v != 0 => Error::IO(err.kind(), msg.to_string()),
                _ => Error::Base(msg.to_string()),
            };
            errors.borrow_mut().insert(level, e);
        });
    }
}

/// Output given message as error level log message.
#[no_mangle]
pub(crate) extern "C" fn bash_error_log(msg: *mut c_char) {
    if let Ok(msg) = unsafe { CStr::from_ptr(msg).to_str() } {
        tracing::error!(msg);
    }
}

/// Output given message as warning level log message.
#[no_mangle]
pub(crate) extern "C" fn bash_warning_log(msg: *mut c_char) {
    if let Ok(msg) = unsafe { CStr::from_ptr(msg).to_str() } {
        tracing::warn!(msg);
    }
}

/// Wrapper to write errors and warning to stderr for interactive mode.
#[no_mangle]
pub(crate) extern "C" fn stderr_output(msg: *mut c_char) {
    let msg = unsafe { CStr::from_ptr(msg).to_string_lossy() };
    eprintln!("{msg}");
}
