use crate::macros::*;
use std::ffi::c_char;

/// Convert a borrowed string to a raw C string.
///
/// Mostly used as a closure function along with crate::macros::iter_to_array.
pub(crate) fn str_to_raw<S: AsRef<str>>(s: S) -> *mut c_char {
    try_ptr_from_str!(s.as_ref())
}

/// Convert an object to a raw pointer.
pub(crate) fn boxed<T>(obj: T) -> *mut T {
    Box::into_raw(Box::new(obj))
}
