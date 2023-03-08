use std::cmp::Ordering;
use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;

use pkgcraft::dep::{Cpv, Dep, Intersects, Version};
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::utils::hash;

use crate::macros::*;

/// Parse a CPV string into a Cpv object.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_new(s: *const c_char) -> *mut Cpv {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let cpv = unwrap_or_return!(Cpv::new(s), ptr::null_mut());
    Box::into_raw(Box::new(cpv))
}

/// Compare two Cpvs returning -1, 0, or 1 if the first is less than, equal to, or
/// greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Cpv pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_cmp(c1: *mut Cpv, c2: *mut Cpv) -> c_int {
    let c1 = null_ptr_check!(c1.as_ref());
    let c2 = null_ptr_check!(c2.as_ref());

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
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_intersects(c1: *mut Cpv, c2: *mut Cpv) -> bool {
    let c1 = null_ptr_check!(c1.as_ref());
    let c2 = null_ptr_check!(c2.as_ref());
    c1.intersects(c2)
}

/// Determine if a Cpv intersects with a package dependency.
///
/// # Safety
/// The arguments must be non-null Cpv and Dep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_intersects_dep(c: *mut Cpv, d: *mut Dep) -> bool {
    let c = null_ptr_check!(c.as_ref());
    let d = null_ptr_check!(d.as_ref());
    c.intersects(d)
}

/// Get the category of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_category(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    CString::new(cpv.category()).unwrap().into_raw()
}

/// Get the package name of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_package(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    CString::new(cpv.package()).unwrap().into_raw()
}

/// Get the version of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_version(c: *mut Cpv) -> *mut Version {
    let cpv = null_ptr_check!(c.as_ref());
    Box::into_raw(Box::new(cpv.version().clone()))
}

/// Get the revision of a Cpv object.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_revision(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    match cpv.revision() {
        None => ptr::null_mut(),
        Some(r) => CString::new(r.as_str()).unwrap().into_raw(),
    }
}

/// Get the package and revision of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_p(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    CString::new(cpv.p()).unwrap().into_raw()
}

/// Get the package, version, and revision of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_pf(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    CString::new(cpv.pf()).unwrap().into_raw()
}

/// Get the revision of a Cpv object.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_pr(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    match cpv.pr().as_str() {
        "" => ptr::null_mut(),
        s => CString::new(s).unwrap().into_raw(),
    }
}

/// Get the version of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_pv(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    CString::new(cpv.pv().as_str()).unwrap().into_raw()
}

/// Get the version and revision of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_pvr(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    CString::new(cpv.pvr().as_str()).unwrap().into_raw()
}

/// Get the category and package of a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_cpn(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    CString::new(cpv.cpn()).unwrap().into_raw()
}

/// Return the string for a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_str(c: *mut Cpv) -> *mut c_char {
    let cpv = null_ptr_check!(c.as_ref());
    CString::new(cpv.to_string()).unwrap().into_raw()
}

/// Return the hash value for a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_hash(c: *mut Cpv) -> u64 {
    let cpv = null_ptr_check!(c.as_ref());
    hash(cpv)
}

/// Return the restriction for a Cpv object.
///
/// # Safety
/// The argument must be a non-null Cpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_restrict(c: *mut Cpv) -> *mut Restrict {
    let cpv = null_ptr_check!(c.as_ref());
    Box::into_raw(Box::new(cpv.into()))
}

/// Determine if a restriction matches a Cpv object.
///
/// # Safety
/// The arguments must be valid Restrict and Cpv pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_restrict_matches(c: *mut Cpv, r: *mut Restrict) -> bool {
    let cpv = null_ptr_check!(c.as_ref());
    let restrict = null_ptr_check!(r.as_ref());
    restrict.matches(cpv)
}

/// Free a Cpv.
///
/// # Safety
/// The argument must be a Cpv pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_free(c: *mut Cpv) {
    if !c.is_null() {
        unsafe { drop(Box::from_raw(c)) };
    }
}
