use std::ffi::{c_char, CStr};
use std::io;

use crate::error::{Error, LAST_ERROR};

/// Wrapper to convert bash errors into native errors.
#[no_mangle]
pub(super) extern "C" fn bash_error(msg: *mut c_char) {
    let msg = unsafe { CStr::from_ptr(msg).to_string_lossy() };

    // Strip the shell name prefix that bash adds -- can't easily do this in bash since the same
    // functionality is used for shell script names which shouldn't be stripped.
    let msg = msg.strip_prefix("scallop: ").unwrap_or(&msg);

    if !msg.is_empty() {
        LAST_ERROR.with(|prev| {
            let err = io::Error::last_os_error();
            // convert bash IO errors into scallop IO errors
            let e = match err.raw_os_error() {
                Some(v) if v != 0 => Error::IO(err.kind(), msg.to_string()),
                _ => Error::Base(msg.to_string()),
            };
            *prev.borrow_mut() = Some(e);
        });
    }
}

/// Output given message as error level log message.
#[no_mangle]
pub(super) extern "C" fn bash_error_log(msg: *mut c_char) {
    if let Ok(msg) = unsafe { CStr::from_ptr(msg).to_str() } {
        tracing::error!(msg);
    }
}

/// Output given message as warning level log message.
#[no_mangle]
pub(super) extern "C" fn bash_warning_log(msg: *mut c_char) {
    if let Ok(msg) = unsafe { CStr::from_ptr(msg).to_str() } {
        tracing::warn!(msg);
    }
}

/// Wrapper to write errors and warning to stderr for interactive mode.
#[no_mangle]
pub(super) extern "C" fn stderr_output(msg: *mut c_char) {
    let msg = unsafe { CStr::from_ptr(msg).to_string_lossy() };
    eprintln!("{msg}");
}
