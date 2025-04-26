use std::cell::RefCell;
use std::ffi::{c_char, CString};
use std::{fmt, ptr};

use crate::macros::*;

#[derive(Debug, Clone)]
#[repr(C)]
pub enum ErrorKind {
    Generic,
    Pkgcraft,
    Config,
    Repo,
    Pkg,
}

#[derive(Debug, Clone)]
pub struct Error {
    message: String,
    kind: ErrorKind,
}

impl Error {
    pub fn new<S: ToString>(s: S) -> Self {
        Error {
            message: s.to_string(),
            kind: ErrorKind::Generic,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}

impl From<pkgcraft::Error> for Error {
    fn from(e: pkgcraft::Error) -> Self {
        use pkgcraft::Error::*;
        let kind = match &e {
            Config(_) => ErrorKind::Config,
            InvalidPkg { .. } => ErrorKind::Pkg,
            InvalidRepo { .. } => ErrorKind::Repo,
            RepoInit(_) => ErrorKind::Repo,
            _ => ErrorKind::Pkgcraft,
        };

        Error { message: e.to_string(), kind }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Error::new(e)
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(e: std::ffi::NulError) -> Self {
        Error::new(e)
    }
}

impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Error::new(e)
    }
}

#[repr(C)]
pub struct PkgcraftError {
    message: *mut c_char,
    kind: ErrorKind,
}

impl From<Error> for PkgcraftError {
    fn from(e: Error) -> Self {
        PkgcraftError {
            message: try_ptr_from_str!(e.message),
            kind: e.kind,
        }
    }
}

impl Drop for PkgcraftError {
    fn drop(&mut self) {
        unsafe {
            drop(CString::from_raw(self.message));
        }
    }
}

thread_local! {
    static LAST_ERROR: RefCell<Option<Error>> = const { RefCell::new(None) };
}

/// Update the most recent error, clearing the previous value.
pub(crate) fn update_last_error<E: Into<Error> + fmt::Debug>(err: E) {
    LAST_ERROR.with(|prev| *prev.borrow_mut() = Some(err.into()));
}

/// Get the most recent error, returns NULL if none exists.
#[unsafe(no_mangle)]
pub extern "C" fn pkgcraft_error_last() -> *mut PkgcraftError {
    match LAST_ERROR.with(|prev| prev.borrow_mut().take()) {
        Some(e) => Box::into_raw(Box::new(e.into())),
        None => ptr::null_mut(),
    }
}

/// Free an error.
///
/// # Safety
/// The argument must be a non-null PkgcraftError pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_error_free(e: *mut PkgcraftError) {
    if !e.is_null() {
        unsafe { drop(Box::from_raw(e)) };
    }
}
