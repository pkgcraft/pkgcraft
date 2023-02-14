use std::ffi::{c_char, CStr};
use std::ptr;

use pkgcraft::dep::{parse, Dep, Version};
use pkgcraft::eapi::{Eapi, IntoEapi};

use crate::macros::*;

/// Parse a package dependency.
///
/// Returns NULL on error.
///
/// # Safety
/// The eapi argument may be NULL to use the default EAPI.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_dep(s: *const c_char, eapi: *const Eapi) -> *const c_char {
    let val = null_ptr_check!(s.as_ref());
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(val).to_str(), ptr::null()) };
    let eapi = unwrap_or_return!(IntoEapi::into_eapi(eapi), ptr::null());
    unwrap_or_return!(Dep::valid(val, eapi), ptr::null());
    s
}

/// Parse a package category.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_category(s: *const c_char) -> *const c_char {
    let val = null_ptr_check!(s.as_ref());
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(val).to_str(), ptr::null()) };
    unwrap_or_return!(parse::category(val), ptr::null());
    s
}

/// Parse a package name.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_package(s: *const c_char) -> *const c_char {
    let val = null_ptr_check!(s.as_ref());
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(val).to_str(), ptr::null()) };
    unwrap_or_return!(parse::package(val), ptr::null());
    s
}

/// Parse a package version.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_version(s: *const c_char) -> *const c_char {
    let val = null_ptr_check!(s.as_ref());
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(val).to_str(), ptr::null()) };
    unwrap_or_return!(Version::valid(val), ptr::null());
    s
}

/// Parse a package repo.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_repo(s: *const c_char) -> *const c_char {
    let val = null_ptr_check!(s.as_ref());
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(val).to_str(), ptr::null()) };
    unwrap_or_return!(parse::repo(val), ptr::null());
    s
}

/// Parse a package CPV.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_cpv(s: *const c_char) -> *const c_char {
    let val = null_ptr_check!(s.as_ref());
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(val).to_str(), ptr::null()) };
    unwrap_or_return!(Dep::valid_cpv(val), ptr::null());
    s
}
