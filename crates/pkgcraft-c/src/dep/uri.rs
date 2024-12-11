use std::ffi::c_char;

use pkgcraft::dep::Uri;

use crate::macros::*;

/// Get the main URI from a Uri object.
///
/// # Safety
/// The argument must be a Uri pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri(u: *mut Uri) -> *mut c_char {
    let uri = try_ref_from_ptr!(u);
    try_ptr_from_str!(uri.as_ref())
}

/// Get the filename for a Uri.
///
/// # Safety
/// The argument must be a Uri pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri_filename(u: *mut Uri) -> *mut c_char {
    let uri = try_ref_from_ptr!(u);
    try_ptr_from_str!(uri.filename())
}

/// Return the formatted string for a Uri object.
///
/// # Safety
/// The argument must be a Uri pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri_str(u: *mut Uri) -> *mut c_char {
    let uri = try_ref_from_ptr!(u);
    try_ptr_from_str!(uri.to_string())
}

/// Free a Uri object.
///
/// # Safety
/// The argument must be a valid Uri pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri_free(u: *mut Uri) {
    if !u.is_null() {
        unsafe { drop(Box::from_raw(u)) };
    }
}
