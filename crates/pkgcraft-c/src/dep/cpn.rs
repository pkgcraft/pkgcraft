use std::cmp::Ordering;
use std::ffi::{c_char, c_int};

use pkgcraft::dep::Cpn;
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Parse a string into a Cpn object.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_new(s: *const c_char) -> *mut Cpn {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let cpn = unwrap_or_panic!(Cpn::try_new(s));
        Box::into_raw(Box::new(cpn))
    }
}

/// Determine if a string is a valid package Cpn.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_parse(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(Cpn::try_new(val));
        s
    }
}

/// Compare two Cpns returning -1, 0, or 1 if the first is less than, equal to, or
/// greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Cpn pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_cmp(c1: *mut Cpn, c2: *mut Cpn) -> c_int {
    let c1 = try_ref_from_ptr!(c1);
    let c2 = try_ref_from_ptr!(c2);

    match c1.cmp(c2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Get the category of a Cpn object.
///
/// # Safety
/// The argument must be a non-null Cpn pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_category(c: *mut Cpn) -> *mut c_char {
    let cpn = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpn.category())
}

/// Get the package name of a Cpn object.
///
/// # Safety
/// The argument must be a non-null Cpn pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_package(c: *mut Cpn) -> *mut c_char {
    let cpn = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpn.package())
}

/// Return the string for a Cpn object.
///
/// # Safety
/// The argument must be a non-null Cpn pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_str(c: *mut Cpn) -> *mut c_char {
    let cpn = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpn.to_string())
}

/// Return the hash value for a Cpn object.
///
/// # Safety
/// The argument must be a non-null Cpn pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_hash(c: *mut Cpn) -> u64 {
    let cpn = try_ref_from_ptr!(c);
    hash(cpn)
}

/// Return the restriction for a Cpn object.
///
/// # Safety
/// The argument must be a non-null Cpn pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_restrict(c: *mut Cpn) -> *mut Restrict {
    let cpn = try_ref_from_ptr!(c);
    Box::into_raw(Box::new(cpn.into()))
}

/// Determine if a restriction matches a Cpn object.
///
/// # Safety
/// The arguments must be valid Restrict and Cpn pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_restrict_matches(c: *mut Cpn, r: *mut Restrict) -> bool {
    let cpn = try_ref_from_ptr!(c);
    let restrict = try_ref_from_ptr!(r);
    restrict.matches(cpn)
}

/// Free a Cpn.
///
/// # Safety
/// The argument must be a Cpn pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpn_free(c: *mut Cpn) {
    if !c.is_null() {
        unsafe { drop(Box::from_raw(c)) };
    }
}
