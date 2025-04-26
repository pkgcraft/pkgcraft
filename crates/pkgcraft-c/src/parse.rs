use std::ffi::c_char;

use pkgcraft::dep::parse;

use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Verify a package category name is valid.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument be a valid UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_parse_category(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(parse::category(val));
        s
    }
}

/// Verify a package name is valid.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument be a valid UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_parse_package(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(parse::package(val));
        s
    }
}

/// Verify a repository name is valid.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument be a valid UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_parse_repo(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(parse::repo(val));
        s
    }
}

/// Verify a package USE flag name is valid.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument be a valid UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_parse_use_flag(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(parse::use_flag(val));
        s
    }
}
