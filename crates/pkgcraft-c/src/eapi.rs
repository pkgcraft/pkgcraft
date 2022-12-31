use std::cmp::Ordering;
use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;
use std::str::FromStr;

use pkgcraft::eapi::{self, Eapi};
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::utils::str_to_raw;

/// Get all known EAPIS.
///
/// # Safety
/// The returned array must be freed via pkgcraft_eapis_free().
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapis(len: *mut usize) -> *mut *const Eapi {
    iter_to_array!(eapi::EAPIS.iter(), len, |&e| { e as *const _ })
}

/// Get all official EAPIS.
///
/// # Safety
/// The returned array must be freed via pkgcraft_eapis_free().
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapis_official(len: *mut usize) -> *mut *const Eapi {
    iter_to_array!(eapi::EAPIS_OFFICIAL.iter(), len, |&e| { e as *const _ })
}

/// Free an array of borrowed Eapi objects.
///
/// # Safety
/// The argument must be the value received from pkgcraft_eapis(), pkgcraft_eapis_official(), or
/// NULL along with the length of the array.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapis_free(eapis: *mut *const Eapi, len: usize) {
    if !eapis.is_null() {
        unsafe { Vec::from_raw_parts(eapis, len, len) };
    }
}

/// Get an EAPI from its identifier.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_from_str(s: *const c_char) -> *const Eapi {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null()) };
    unwrap_or_return!(<&Eapi>::from_str(s), ptr::null())
}

/// Check if an EAPI has a feature.
///
/// # Safety
/// The arguments must be a non-null Eapi pointer and non-null string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_has(eapi: *const Eapi, s: *const c_char) -> bool {
    let eapi = null_ptr_check!(eapi.as_ref());
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), false) };
    match eapi::Feature::from_str(s) {
        Ok(feature) => eapi.has(feature),
        _ => false,
    }
}

/// Return an EAPI's identifier.
///
/// # Safety
/// The arguments must be a non-null Eapi pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_as_str(eapi: *const Eapi) -> *mut c_char {
    let eapi = null_ptr_check!(eapi.as_ref());
    CString::new(eapi.as_str()).unwrap().into_raw()
}

/// Compare two Eapi objects chronologically returning -1, 0, or 1 if the first is less than, equal
/// to, or greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Eapi pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_cmp(e1: *const Eapi, e2: *const Eapi) -> c_int {
    let eapi1 = null_ptr_check!(e1.as_ref());
    let eapi2 = null_ptr_check!(e2.as_ref());

    match eapi1.cmp(eapi2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the array of dependency keys for an Eapi.
///
/// # Safety
/// The argument must be a non-null Eapi pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_dep_keys(
    eapi: *const Eapi,
    len: *mut usize,
) -> *mut *mut c_char {
    let eapi = null_ptr_check!(eapi.as_ref());
    iter_to_array!(eapi.dep_keys().iter(), len, str_to_raw)
}

/// Return the array of metadata keys for an Eapi.
///
/// # Safety
/// The argument must be a non-null Eapi pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_metadata_keys(
    eapi: *const Eapi,
    len: *mut usize,
) -> *mut *mut c_char {
    let eapi = null_ptr_check!(eapi.as_ref());
    iter_to_array!(eapi.metadata_keys().iter(), len, str_to_raw)
}

/// Return the hash value for an Eapi.
///
/// # Safety
/// The argument must be a non-null Eapi pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_hash(eapi: *const Eapi) -> u64 {
    let eapi = null_ptr_check!(eapi.as_ref());
    hash(eapi)
}

/// Convert EAPI range into an array of Eapi objects.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapis_range(
    s: *const c_char,
    len: *mut usize,
) -> *mut *const Eapi {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let eapis = unwrap_or_return!(eapi::range(s), ptr::null_mut());
    iter_to_array!(eapis, len, |e| { e as *const _ })
}
