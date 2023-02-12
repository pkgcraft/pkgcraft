use std::cmp::Ordering;
use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;
use std::str::FromStr;

use pkgcraft::dep::{Blocker, PkgDep, SlotOperator, Version};
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
pub unsafe extern "C" fn pkgcraft_dep_new(s: *const c_char, eapi: *const Eapi) -> *mut PkgDep {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let eapi = unwrap_or_return!(IntoEapi::into_eapi(eapi), ptr::null_mut());
    let dep = unwrap_or_return!(PkgDep::new(s, eapi), ptr::null_mut());
    Box::into_raw(Box::new(dep))
}

/// Parse a CPV string into a package dependency.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_new(s: *const c_char) -> *mut PkgDep {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let cpv = unwrap_or_return!(PkgDep::new_cpv(s), ptr::null_mut());
    Box::into_raw(Box::new(cpv))
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
/// The arguments must be non-null PkgDep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_cmp(d1: *mut PkgDep, d2: *mut PkgDep) -> c_int {
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
/// The arguments must be non-null PkgDep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_intersects(d1: *mut PkgDep, d2: *mut PkgDep) -> bool {
    let d1 = null_ptr_check!(d1.as_ref());
    let d2 = null_ptr_check!(d2.as_ref());
    d1.intersects(d2)
}

/// Return a package dependency's category.
/// For example, the package dependency "=cat/pkg-1-r2" returns "cat".
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_category(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.category()).unwrap().into_raw()
}

/// Return an package dependency's package.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg".
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_package(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.package()).unwrap().into_raw()
}

/// Return a package dependency's blocker.
/// For example, the package dependency "!cat/pkg" has a weak blocker.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_blocker(d: *mut PkgDep) -> Blocker {
    let dep = null_ptr_check!(d.as_ref());
    dep.blocker().unwrap_or_default()
}

/// Return a package dependency's version.
/// For example, the package dependency "=cat/pkg-1-r2" returns "1-r2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_version(d: *mut PkgDep) -> *mut Version {
    let dep = null_ptr_check!(d.as_ref());
    match dep.version() {
        None => ptr::null_mut(),
        Some(v) => Box::into_raw(Box::new(v.clone())),
    }
}

/// Return a package dependency's revision.
/// For example, the package dependency "=cat/pkg-1-r2" returns "2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_revision(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.revision() {
        None => ptr::null_mut(),
        Some(r) => CString::new(r.as_str()).unwrap().into_raw(),
    }
}

/// Return a package dependency's slot.
/// For example, the package dependency "=cat/pkg-1-r2:3" returns "3".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_slot(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.slot() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Return a package dependency's subslot.
/// For example, the package dependency "=cat/pkg-1-r2:3/4" returns "4".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_subslot(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.subslot() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Return a package dependency's slot operator.
/// For example, the package dependency "=cat/pkg-1-r2:0=" has an equal slot operator.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_slot_op(d: *mut PkgDep) -> SlotOperator {
    let dep = null_ptr_check!(d.as_ref());
    dep.slot_op().unwrap_or_default()
}

/// Return a package dependency's USE dependencies.
/// For example, the package dependency "=cat/pkg-1-r2[a,b,c]" has USE dependencies of "a, b, c".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_use_deps(
    d: *mut PkgDep,
    len: *mut usize,
) -> *mut *mut c_char {
    // TODO: switch from usize to std::os::raw::c_size_t when it's stable.
    let dep = null_ptr_check!(d.as_ref());
    match dep.use_deps() {
        None => ptr::null_mut(),
        Some(use_deps) => iter_to_array!(use_deps.iter(), len, str_to_raw),
    }
}

/// Return a package dependency's repo.
/// For example, the package dependency "=cat/pkg-1-r2:3/4::repo" returns "repo".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_repo(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.repo() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Return the package name and version.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1".
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_p(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.p()).unwrap().into_raw()
}

/// Return the package name, version, and revision.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1-r2".
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_pf(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.pf()).unwrap().into_raw()
}

/// Return the package dependency's revision.
/// For example, the package dependency "=cat/pkg-1-r2" returns "r2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_pr(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.pr().as_str() {
        "" => ptr::null_mut(),
        s => CString::new(s).unwrap().into_raw(),
    }
}

/// Return the package dependency's version.
/// For example, the package dependency "=cat/pkg-1-r2" returns "1".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_pv(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.pv().as_str() {
        "" => ptr::null_mut(),
        s => CString::new(s).unwrap().into_raw(),
    }
}

/// Return the package dependency's version and revision.
/// For example, the package dependency "=cat/pkg-1-r2" returns "1-r2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_pvr(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    match dep.pvr().as_str() {
        "" => ptr::null_mut(),
        s => CString::new(s).unwrap().into_raw(),
    }
}

/// Return the package dependency's category and package.
/// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg".
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_cpn(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.cpn()).unwrap().into_raw()
}

/// Return the package dependency's category, package, version, and revision.
/// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg-1-r2".
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_cpv(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.cpv()).unwrap().into_raw()
}

/// Return the string for a package dependency.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_str(d: *mut PkgDep) -> *mut c_char {
    let dep = null_ptr_check!(d.as_ref());
    CString::new(dep.to_string()).unwrap().into_raw()
}

/// Return the hash value for a package dependency.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_hash(d: *mut PkgDep) -> u64 {
    let dep = null_ptr_check!(d.as_ref());
    hash(dep)
}

/// Return the restriction for a package dependency.
///
/// # Safety
/// The argument must be a non-null PkgDep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_restrict(d: *mut PkgDep) -> *mut Restrict {
    let dep = null_ptr_check!(d.as_ref());
    Box::into_raw(Box::new(dep.into()))
}

/// Determine if a restriction matches a package dependency.
///
/// # Safety
/// The arguments must be valid Restrict and PkgDep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_restrict_matches(d: *mut PkgDep, r: *mut Restrict) -> bool {
    let dep = null_ptr_check!(d.as_ref());
    let restrict = null_ptr_check!(r.as_ref());
    restrict.matches(dep)
}

/// Free a package dependency.
///
/// # Safety
/// The argument must be a PkgDep pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_free(d: *mut PkgDep) {
    if !d.is_null() {
        unsafe { drop(Box::from_raw(d)) };
    }
}
