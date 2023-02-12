use std::cmp::Ordering;
use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;
use std::str::FromStr;

use pkgcraft::dep::{Operator, Version};
use pkgcraft::utils::hash;

use crate::macros::*;

/// Parse a string into a version.
///
/// Returns NULL on error.
///
/// # Safety
/// The version argument should point to a valid string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_new(s: *const c_char) -> *mut Version {
    let ver_str = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let ver = unwrap_or_return!(Version::new(ver_str), ptr::null_mut());
    Box::into_raw(Box::new(ver))
}

/// Parse a string into a version with an operator.
///
/// Returns NULL on error.
///
/// # Safety
/// The version argument should point to a valid string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_with_op(s: *const c_char) -> *mut Version {
    let ver_str = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let ver = unwrap_or_return!(Version::new_with_op(ver_str), ptr::null_mut());
    Box::into_raw(Box::new(ver))
}

/// Return a version's operator.
///
/// # Safety
/// The argument must be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_op(v: *mut Version) -> Operator {
    let ver = null_ptr_check!(v.as_ref());
    ver.op().unwrap_or_default()
}

/// Return a version's base, e.g. the version "1-r2" has a base of "1".
///
/// # Safety
/// The version argument should be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_base(v: *mut Version) -> *mut c_char {
    let ver = null_ptr_check!(v.as_ref());
    CString::new(ver.base()).unwrap().into_raw()
}

/// Parse a string into an Operator.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_op_from_str(s: *const c_char) -> Operator {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), Operator::NONE) };
    Operator::from_str(s).unwrap_or_default()
}

/// Return the string for an Operator.
#[no_mangle]
pub extern "C" fn pkgcraft_version_op_str(op: Operator) -> *mut c_char {
    CString::new(op.as_ref()).unwrap().into_raw()
}

/// Compare two versions returning -1, 0, or 1 if the first version is less than, equal to, or greater
/// than the second version, respectively.
///
/// # Safety
/// The version arguments should be non-null Version pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_cmp(v1: *mut Version, v2: *mut Version) -> c_int {
    let v1 = null_ptr_check!(v1.as_ref());
    let v2 = null_ptr_check!(v2.as_ref());

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
    let v1 = null_ptr_check!(v1.as_ref());
    let v2 = null_ptr_check!(v2.as_ref());
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
    let ver = null_ptr_check!(v.as_ref());
    match ver.revision() {
        Some(r) => CString::new(r.as_str()).unwrap().into_raw(),
        None => ptr::null_mut(),
    }
}

/// Return a version's string value without operator.
///
/// # Safety
/// The version argument should be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_str(v: *mut Version) -> *mut c_char {
    let ver = null_ptr_check!(v.as_ref());
    CString::new(ver.as_str()).unwrap().into_raw()
}

/// Return a version's string value including the operator if it exists.
///
/// # Safety
/// The version argument should be a non-null Version pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_version_str_with_op(v: *mut Version) -> *mut c_char {
    let ver = null_ptr_check!(v.as_ref());
    CString::new(ver.to_string_with_op()).unwrap().into_raw()
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
    let ver = null_ptr_check!(v.as_ref());
    hash(ver)
}
