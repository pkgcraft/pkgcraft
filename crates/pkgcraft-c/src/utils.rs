use std::ffi::{c_char, CString};

/// Convert a str reference to a raw C string.
///
/// Mostly used as a closure function along with crate::macros::iter_to_array.
pub(crate) fn str_to_raw<S: AsRef<str>>(s: S) -> *mut c_char {
    CString::new(s.as_ref()).unwrap().into_raw()
}
