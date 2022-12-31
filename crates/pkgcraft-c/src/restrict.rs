use std::ffi::{c_char, CStr};
use std::ptr;

use pkgcraft::restrict::{parse, Restrict};
use pkgcraft::utils::hash;

use crate::macros::*;

/// Parse a dependency restriction.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_restrict_parse_dep(s: *const c_char) -> *mut Restrict {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let restrict = unwrap_or_return!(parse::dep(s), ptr::null_mut());
    Box::into_raw(Box::new(restrict))
}

/// Parse a package query restriction.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_restrict_parse_pkg(s: *const c_char) -> *mut Restrict {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let restrict = unwrap_or_return!(parse::pkg(s), ptr::null_mut());
    Box::into_raw(Box::new(restrict))
}

/// Determine if two restrictions are equal.
///
/// # Safety
/// The arguments must be non-null Restrict pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_restrict_eq(r1: *mut Restrict, r2: *mut Restrict) -> bool {
    let r1 = null_ptr_check!(r1.as_ref());
    let r2 = null_ptr_check!(r2.as_ref());
    r1.eq(r2)
}

/// Return the hash value for a restriction.
///
/// # Safety
/// The argument must be a non-null Restrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_restrict_hash(r: *mut Restrict) -> u64 {
    let restrict = null_ptr_check!(r.as_ref());
    hash(restrict)
}

/// Create a new restriction combining two restrictions via logical AND.
///
/// # Safety
/// The arguments must be Restrict pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_restrict_and(
    r1: *mut Restrict,
    r2: *mut Restrict,
) -> *mut Restrict {
    let r1 = null_ptr_check!(r1.as_ref());
    let r2 = null_ptr_check!(r2.as_ref());
    Box::into_raw(Box::new(r1.clone() & r2.clone()))
}

/// Create a new restriction combining two restrictions via logical OR.
///
/// # Safety
/// The arguments must be Restrict pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_restrict_or(
    r1: *mut Restrict,
    r2: *mut Restrict,
) -> *mut Restrict {
    let r1 = null_ptr_check!(r1.as_ref());
    let r2 = null_ptr_check!(r2.as_ref());
    Box::into_raw(Box::new(r1.clone() | r2.clone()))
}

/// Create a new restriction combining two restrictions via logical XOR.
///
/// # Safety
/// The arguments must be Restrict pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_restrict_xor(
    r1: *mut Restrict,
    r2: *mut Restrict,
) -> *mut Restrict {
    let r1 = null_ptr_check!(r1.as_ref());
    let r2 = null_ptr_check!(r2.as_ref());
    Box::into_raw(Box::new(r1.clone() ^ r2.clone()))
}

/// Create a new restriction inverting a restriction via logical NOT.
///
/// # Safety
/// The arguments must be a Restrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_restrict_not(r: *mut Restrict) -> *mut Restrict {
    let r = null_ptr_check!(r.as_ref());
    Box::into_raw(Box::new(!r.clone()))
}

/// Free a restriction.
///
/// # Safety
/// The argument must be a Restrict pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_restrict_free(r: *mut Restrict) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}
