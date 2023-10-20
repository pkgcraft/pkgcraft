use std::cmp::Ordering;
use std::ffi::{c_char, c_int};
use std::ptr;

use pkgcraft::dep::{Intersects, Operator, Version};
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Parse a string into a version.
///
/// Returns NULL on error.
///
/// # Safety
/// The version argument should point to a valid string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_new(s: *const c_char) -> *mut Version {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let ver = unwrap_or_panic!(Version::new(s));
        Box::into_raw(Box::new(ver))
    }
}

/// Return a version's operator.
///
/// # Safety
/// The argument must be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_op(v: *mut Version) -> Operator {
    let ver = try_ref_from_ptr!(v);
    ver.op().unwrap_or_default()
}

/// Return a version's base, e.g. the version "1-r2" has a base of "1".
///
/// # Safety
/// The version argument should be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_base(v: *mut Version) -> *mut c_char {
    let ver = try_ref_from_ptr!(v);
    try_ptr_from_str!(ver.base())
}

/// Parse a string into an Operator.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_op_from_str(s: *const c_char) -> Operator {
    let s = try_str_from_ptr!(s);
    s.parse().unwrap_or_default()
}

/// Return the string for an Operator.
#[no_mangle]
pub extern "C" fn pkgcraft_version_op_str(op: Operator) -> *mut c_char {
    try_ptr_from_str!(op.as_ref())
}

/// Compare two versions returning -1, 0, or 1 if the first version is less than, equal to, or greater
/// than the second version, respectively.
///
/// # Safety
/// The version arguments should be non-null Version pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_cmp(v1: *mut Version, v2: *mut Version) -> c_int {
    let v1 = try_ref_from_ptr!(v1);
    let v2 = try_ref_from_ptr!(v2);

    match v1.cmp(v2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Determine if two versions intersect.
///
/// # Safety
/// The version arguments should be non-null Version pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_intersects(v1: *mut Version, v2: *mut Version) -> bool {
    let v1 = try_ref_from_ptr!(v1);
    let v2 = try_ref_from_ptr!(v2);
    v1.intersects(v2)
}

/// Return a version's revision, e.g. the version "1-r2" has a revision of "2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The version argument should be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_revision(v: *mut Version) -> *mut c_char {
    let ver = try_ref_from_ptr!(v);
    match ver.revision() {
        Some(r) => try_ptr_from_str!(r.as_str()),
        None => ptr::null_mut(),
    }
}

/// Return a version's string value without operator.
///
/// # Safety
/// The version argument should be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_str(v: *mut Version) -> *mut c_char {
    let ver = try_ref_from_ptr!(v);
    try_ptr_from_str!(ver.to_string())
}

/// Free a version.
///
/// # Safety
/// The version argument should be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_free(v: *mut Version) {
    if !v.is_null() {
        let _ = unsafe { Box::from_raw(v) };
    }
}

/// Return the hash value for a version.
///
/// # Safety
/// The version argument should be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_hash(v: *mut Version) -> u64 {
    let ver = try_ref_from_ptr!(v);
    hash(ver)
}
