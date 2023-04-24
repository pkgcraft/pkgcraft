use std::cmp::Ordering;
use std::ffi::{c_char, c_int};
use std::str::FromStr;

use pkgcraft::eapi::{self, Eapi};
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::panic::ffi_catch_panic;
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

/// Get an EAPI from its identifier.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_from_str(s: *const c_char) -> *const Eapi {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        unwrap_or_panic!(<&Eapi>::from_str(s))
    }
}

/// Check if an EAPI has a feature.
///
/// # Safety
/// The arguments must be a non-null Eapi pointer and non-null string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_has(eapi: *const Eapi, s: *const c_char) -> bool {
    let eapi = try_ref_from_ptr!(eapi);
    let s = try_str_from_ptr!(s);
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
    let eapi = try_ref_from_ptr!(eapi);
    try_ptr_from_str!(eapi.as_str())
}

/// Compare two Eapi objects chronologically returning -1, 0, or 1 if the first is less than, equal
/// to, or greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Eapi pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_cmp(e1: *const Eapi, e2: *const Eapi) -> c_int {
    let eapi1 = try_ref_from_ptr!(e1);
    let eapi2 = try_ref_from_ptr!(e2);

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
    let eapi = try_ref_from_ptr!(eapi);
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
    let eapi = try_ref_from_ptr!(eapi);
    iter_to_array!(eapi.metadata_keys().iter(), len, str_to_raw)
}

/// Return the hash value for an Eapi.
///
/// # Safety
/// The argument must be a non-null Eapi pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_eapi_hash(eapi: *const Eapi) -> u64 {
    let eapi = try_ref_from_ptr!(eapi);
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
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let eapis = unwrap_or_panic!(eapi::range(s));
        iter_to_array!(eapis, len, |e| { e as *const _ })
    }
}
