use std::ffi::{c_char, CStr};
use std::ptr;

use pkgcraft::atom::{parse, Atom, Version};
use pkgcraft::eapi::{Eapi, IntoEapi};

use crate::macros::*;

/// Parse an atom string.
///
/// Returns NULL on error.
///
/// # Safety
/// The atom argument should be a UTF-8 string while eapi can be a string or may be
/// NULL to use the default EAPI.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_atom(
    atom: *const c_char,
    eapi: *const Eapi,
) -> *const c_char {
    let s = null_ptr_check!(atom.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null()) };
    let eapi = unwrap_or_return!(IntoEapi::into_eapi(eapi), ptr::null());
    unwrap_or_return!(Atom::valid(s, eapi), ptr::null());
    atom
}

/// Parse an atom category string.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_category(s: *const c_char) -> *const c_char {
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null()) };
    unwrap_or_return!(parse::category(val), ptr::null());
    s
}

/// Parse an atom package string.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_package(s: *const c_char) -> *const c_char {
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null()) };
    unwrap_or_return!(parse::package(val), ptr::null());
    s
}

/// Parse an atom version string.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_version(s: *const c_char) -> *const c_char {
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null()) };
    unwrap_or_return!(Version::valid(val), ptr::null());
    s
}

/// Parse an atom repo string.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_repo(s: *const c_char) -> *const c_char {
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null()) };
    unwrap_or_return!(parse::repo(val), ptr::null());
    s
}

/// Parse a CPV string.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_cpv(s: *const c_char) -> *const c_char {
    let val = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null()) };
    unwrap_or_return!(Atom::valid_cpv(val), ptr::null());
    s
}
