use std::cmp::Ordering;
use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;
use std::str::FromStr;

use pkgcraft::dep::{Blocker, Cpv, Dep, Intersects, SlotOperator, Version};
use pkgcraft::eapi::{Eapi, IntoEapi};
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::utils::str_to_raw;

/// Parse a string into a package dependency using a specific EAPI. Pass NULL for the eapi argument
/// in order to parse using the latest EAPI with extensions (e.g. support for repo deps).
///
/// Returns NULL on error.
///
/// # Safety
/// The eapi argument may be NULL to use the default EAPI.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_new(s: *const c_char, eapi: *const Eapi) -> *mut Dep {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let eapi = unwrap_or_return!(IntoEapi::into_eapi(eapi), ptr::null_mut());
    let dep = unwrap_or_return!(Dep::new(s, eapi), ptr::null_mut());
    Box::into_raw(Box::new(dep))
}

/// Parse a string into a Blocker.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_blocker_from_str(s: *const c_char) -> Blocker {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), Blocker::NONE) };
    Blocker::from_str(s).unwrap_or_default()
}

/// Return the string for a Blocker.
#[no_mangle]
pub extern "C" fn pkgcraft_dep_blocker_str(b: Blocker) -> *mut c_char {
    CString::new(b.as_ref()).unwrap().into_raw()
}

/// Parse a string into a SlotOperator.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_slot_op_from_str(s: *const c_char) -> SlotOperator {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), SlotOperator::NONE) };
    SlotOperator::from_str(s).unwrap_or_default()
}

/// Return the string for a SlotOperator.
#[no_mangle]
pub extern "C" fn pkgcraft_dep_slot_op_str(op: SlotOperator) -> *mut c_char {
    CString::new(op.as_ref()).unwrap().into_raw()
}

/// Compare two package dependencies returning -1, 0, or 1 if the first is less than, equal to, or
/// greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Dep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_cmp(d1: *mut Dep, d2: *mut Dep) -> c_int {
    let d1 = null_ptr_check!(d1.as_ref());
    let d2 = null_ptr_check!(d2.as_ref());

    match d1.cmp(d2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Determine if two package dependencies intersect.
///
/// # Safety
/// The arguments must be non-null Dep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_intersects(d1: *mut Dep, d2: *mut Dep) -> bool {
    let d1 = null_ptr_check!(d1.as_ref());
    let d2 = null_ptr_check!(d2.as_ref());
    d1.intersects(d2)
}

/// Determine if a package dependency intersects with a Cpv.
///
/// # Safety
/// The arguments must be non-null Cpv and Dep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_intersects_cpv(d: *mut Dep, c: *mut Cpv) -> bool {
    let d = null_ptr_check!(d.as_ref());
    let c = null_ptr_check!(c.as_ref());
    d.intersects(c)
}

/// Get the category of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "cat".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_category(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.category()).unwrap().into_raw()
}

/// Get the package name of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_package(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.package()).unwrap().into_raw()
}

/// Get the blocker of a package dependency.
/// For example, the package dependency "!cat/pkg" has a weak blocker.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_blocker(d: *mut Dep) -> Blocker {
    let dep = null_ptr_check!(d.as_ref());
    dep.blocker().unwrap_or_default()
}

/// Get the version of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "1-r2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_version(d: *mut Dep) -> *mut Version {
    let dep = null_ptr_check!(d.as_ref());
    match dep.version() {
        None => ptr::null_mut(),
        Some(v) => Box::into_raw(Box::new(v.clone())),
    }
}

/// Get the revision of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_revision(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.revision() {
        None => ptr::null_mut(),
        Some(r) => CString::new(r.as_str()).unwrap().into_raw(),
    }
}

/// Get the slot of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2:3" returns "3".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_slot(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.slot() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Get the subslot of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2:3/4" returns "4".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_subslot(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.subslot() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Get the slot operator of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2:0=" has an equal slot operator.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_slot_op(d: *mut Dep) -> SlotOperator {
    let dep = null_ptr_check!(d.as_ref());
    dep.slot_op().unwrap_or_default()
}

/// Get the USE dependencies of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2[a,b,c]" has USE dependencies of "a, b, c".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_use_deps(d: *mut Dep, len: *mut usize) -> *mut *mut c_char {
    // TODO: switch from usize to std::os::raw::c_size_t when it's stable.
    let dep = null_ptr_check!(d.as_ref());
    match dep.use_deps() {
        None => ptr::null_mut(),
        Some(use_deps) => iter_to_array!(use_deps.iter(), len, str_to_raw),
    }
}

/// Get the repo of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2:3/4::repo" returns "repo".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_repo(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.repo() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Get the package and revision of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_p(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.p()).unwrap().into_raw()
}

/// Get the package, version, and revision of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1-r2".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_pf(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.pf()).unwrap().into_raw()
}

/// Get the revision of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "r2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_pr(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.pr().as_str() {
        "" => ptr::null_mut(),
        s => CString::new(s).unwrap().into_raw(),
    }
}

/// Get the version of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "1".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_pv(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.pv().as_str() {
        "" => ptr::null_mut(),
        s => CString::new(s).unwrap().into_raw(),
    }
}

/// Get the version and revision of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "1-r2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_pvr(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.pvr().as_str() {
        "" => ptr::null_mut(),
        s => CString::new(s).unwrap().into_raw(),
    }
}

/// Get the category and package of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_cpn(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.cpn()).unwrap().into_raw()
}

/// Get the category, package, and version of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg-1-r2".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_cpv(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.cpv()).unwrap().into_raw()
}

/// Return the string for a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_str(d: *mut Dep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.to_string()).unwrap().into_raw()
}

/// Return the hash value for a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_hash(d: *mut Dep) -> u64 {
    let dep = null_ptr_check!(d.as_ref());
    hash(dep)
}

/// Return the restriction for a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_restrict(d: *mut Dep) -> *mut Restrict {
    let dep = null_ptr_check!(d.as_ref());
    Box::into_raw(Box::new(dep.into()))
}

/// Determine if a restriction matches a package dependency.
///
/// # Safety
/// The arguments must be valid Restrict and Dep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_restrict_matches(d: *mut Dep, r: *mut Restrict) -> bool {
    let dep = null_ptr_check!(d.as_ref());
    let restrict = null_ptr_check!(r.as_ref());
    restrict.matches(dep)
}

/// Free a package dependency.
///
/// # Safety
/// The argument must be a Dep pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_free(d: *mut Dep) {
    if !d.is_null() {
        unsafe { drop(Box::from_raw(d)) };
    }
}
