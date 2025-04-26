use std::ffi::c_char;

use pkgcraft::restrict::{Restrict, parse};
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Parse a dependency restriction.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_restrict_parse_dep(s: *const c_char) -> *mut Restrict {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let restrict = unwrap_or_panic!(parse::dep(s));
        Box::into_raw(Box::new(restrict))
    }
}

/// Parse a package query restriction.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_restrict_parse_pkg(s: *const c_char) -> *mut Restrict {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let restrict = unwrap_or_panic!(parse::pkg(s));
        Box::into_raw(Box::new(restrict))
    }
}

/// Determine if two restrictions are equal.
///
/// # Safety
/// The arguments must be non-null Restrict pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_restrict_eq(r1: *mut Restrict, r2: *mut Restrict) -> bool {
    let r1 = try_ref_from_ptr!(r1);
    let r2 = try_ref_from_ptr!(r2);
    r1.eq(r2)
}

/// Return the hash value for a restriction.
///
/// # Safety
/// The argument must be a non-null Restrict pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_restrict_hash(r: *mut Restrict) -> u64 {
    let restrict = try_ref_from_ptr!(r);
    hash(restrict)
}

/// Create a new restriction combining two restrictions via logical AND.
///
/// # Safety
/// The arguments must be Restrict pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_restrict_and(
    r1: *mut Restrict,
    r2: *mut Restrict,
) -> *mut Restrict {
    let r1 = try_ref_from_ptr!(r1);
    let r2 = try_ref_from_ptr!(r2);
    Box::into_raw(Box::new(r1.clone() & r2.clone()))
}

/// Create a new restriction combining two restrictions via logical OR.
///
/// # Safety
/// The arguments must be Restrict pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_restrict_or(
    r1: *mut Restrict,
    r2: *mut Restrict,
) -> *mut Restrict {
    let r1 = try_ref_from_ptr!(r1);
    let r2 = try_ref_from_ptr!(r2);
    Box::into_raw(Box::new(r1.clone() | r2.clone()))
}

/// Create a new restriction combining two restrictions via logical XOR.
///
/// # Safety
/// The arguments must be Restrict pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_restrict_xor(
    r1: *mut Restrict,
    r2: *mut Restrict,
) -> *mut Restrict {
    let r1 = try_ref_from_ptr!(r1);
    let r2 = try_ref_from_ptr!(r2);
    Box::into_raw(Box::new(r1.clone() ^ r2.clone()))
}

/// Create a new restriction inverting a restriction via logical NOT.
///
/// # Safety
/// The arguments must be a Restrict pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_restrict_not(r: *mut Restrict) -> *mut Restrict {
    let r = try_ref_from_ptr!(r);
    Box::into_raw(Box::new(!r.clone()))
}

/// Free a restriction.
///
/// # Safety
/// The argument must be a Restrict pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_restrict_free(r: *mut Restrict) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}
