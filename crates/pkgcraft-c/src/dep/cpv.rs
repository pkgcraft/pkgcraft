use std::cmp::Ordering;
use std::ffi::{c_char, c_int};
use std::ptr;

use pkgcraft::dep::version::{Operator, WithOp};
use pkgcraft::dep::{Cpn, Cpv, Dep, Version};
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::traits::Intersects;
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Parse a string into a Cpv object.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_new(s: *const c_char) -> *mut Cpv {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let cpv = unwrap_or_panic!(Cpv::try_new(s));
        Box::into_raw(Box::new(cpv))
    }
}

/// Determine if a string is a valid package Cpv.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should point to a UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_parse(s: *const c_char) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        unwrap_or_panic!(Cpv::try_new(val));
        s
    }
}

/// Create a Dep from a Cpv by applying a version operator.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_with_op(c: *mut Cpv, op: Operator) -> *mut Dep {
    ffi_catch_panic! {
        let cpv = try_ref_from_ptr!(c);
        let dep = unwrap_or_panic!(cpv.clone().with_op(op));
        Box::into_raw(Box::new(dep))
    }
}

/// Compare two Cpvs returning -1, 0, or 1 if the first is less than, equal to, or
/// greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Cpv pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_cmp(c1: *mut Cpv, c2: *mut Cpv) -> c_int {
    let c1 = try_ref_from_ptr!(c1);
    let c2 = try_ref_from_ptr!(c2);

    match c1.cmp(c2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Determine if two Cpv objects intersect.
///
/// # Safety
/// The arguments must be non-null Cpv pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_intersects(c1: *mut Cpv, c2: *mut Cpv) -> bool {
    let c1 = try_ref_from_ptr!(c1);
    let c2 = try_ref_from_ptr!(c2);
    c1.intersects(c2)
}

/// Determine if a Cpv intersects with a package dependency.
///
/// # Safety
/// The arguments must be non-null Cpv and Dep pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_intersects_dep(c: *mut Cpv, d: *mut Dep) -> bool {
    let c = try_ref_from_ptr!(c);
    let d = try_ref_from_ptr!(d);
    c.intersects(d)
}

/// Get the category of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_category(c: *mut Cpv) -> *mut c_char {
    let cpv = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpv.category())
}

/// Get the package name of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_package(c: *mut Cpv) -> *mut c_char {
    let cpv = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpv.package())
}

/// Get the version of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_version(c: *mut Cpv) -> *mut Version {
    let cpv = try_ref_from_ptr!(c);
    Box::into_raw(Box::new(cpv.version().clone()))
}

/// Get the package and revision of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_p(c: *mut Cpv) -> *mut c_char {
    let cpv = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpv.p())
}

/// Get the package, version, and revision of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_pf(c: *mut Cpv) -> *mut c_char {
    let cpv = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpv.pf())
}

/// Get the revision of a Cpv object.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_pr(c: *mut Cpv) -> *mut c_char {
    let cpv = try_ref_from_ptr!(c);
    match cpv.pr().as_str() {
        "" => ptr::null_mut(),
        s => try_ptr_from_str!(s),
    }
}

/// Get the version of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_pv(c: *mut Cpv) -> *mut c_char {
    let cpv = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpv.pv())
}

/// Get the version and revision of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_pvr(c: *mut Cpv) -> *mut c_char {
    let cpv = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpv.pvr())
}

/// Get the Cpn of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_cpn(c: *mut Cpv) -> *mut Cpn {
    let cpv = try_ref_from_ptr!(c);
    Box::into_raw(Box::new(cpv.cpn().clone()))
}

/// Return the string for a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_str(c: *mut Cpv) -> *mut c_char {
    let cpv = try_ref_from_ptr!(c);
    try_ptr_from_str!(cpv.to_string())
}

/// Return the hash value for a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_hash(c: *mut Cpv) -> u64 {
    let cpv = try_ref_from_ptr!(c);
    hash(cpv)
}

/// Return the restriction for a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_restrict(c: *mut Cpv) -> *mut Restrict {
    let cpv = try_ref_from_ptr!(c);
    Box::into_raw(Box::new(cpv.into()))
}

/// Determine if a restriction matches a Cpv object.
///
/// # Safety
/// The arguments must be valid Restrict and Cpv pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_restrict_matches(c: *mut Cpv, r: *mut Restrict) -> bool {
    let cpv = try_ref_from_ptr!(c);
    let restrict = try_ref_from_ptr!(r);
    restrict.matches(cpv)
}

/// Free a Cpv.
///
/// # Safety
/// The argument must be a Cpv pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_cpv_free(c: *mut Cpv) {
    if !c.is_null() {
        unsafe { drop(Box::from_raw(c)) };
    }
}
