use std::ffi::c_char;

use pkgcraft::dep::{parse, Cpv, Version};

use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Parse a package category.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_category(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(parse::category(val));
        s
    }
}

/// Parse a package name.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_package(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(parse::package(val));
        s
    }
}

/// Parse a package version.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_version(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(Version::valid(val));
        s
    }
}

/// Parse a package repo.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_repo(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(parse::repo(val));
        s
    }
}

/// Parse a package CPV.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_parse_cpv(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(Cpv::valid(val));
        s
    }
}
