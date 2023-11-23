use std::borrow::Cow;
use std::cmp::Ordering;
use std::ffi::{c_char, c_int};
use std::ptr;

use pkgcraft::dep::{Blocker, Cpv, Dep, DepFields, Intersects, SlotOperator, Version};
use pkgcraft::eapi::Eapi;
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::utils::hash;

use crate::eapi::eapi_or_default;
use crate::macros::*;
use crate::panic::ffi_catch_panic;
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
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let eapi = eapi_or_default!(eapi);
        let dep = unwrap_or_panic!(eapi.dep(s));
        Box::into_raw(Box::new(dep))
    }
}

/// Determine if a string is a valid package dependency.
///
/// Returns NULL on error.
///
/// # Safety
/// The eapi argument may be NULL to use the default EAPI.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_valid(s: *const c_char, eapi: *const Eapi) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        let eapi = option_from_ptr!(eapi);
        unwrap_or_panic!(Dep::valid(val, eapi));
        s
    }
}

/// Return a given package dependency without the specified fields.
///
/// # Safety
/// The arguments must be a non-null Dep pointer and a DepFields bitflag.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_without(d: *mut Dep, fields: u32) -> *mut Dep {
    let dep = try_ref_from_ptr!(d);
    let fields = DepFields::from_bits_truncate(fields);
    if let Cow::Owned(d) = dep.without(fields) {
        Box::into_raw(Box::new(d))
    } else {
        d
    }
}

/// Return a given package dependency with the specified repo name.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be a non-null Dep pointer and valid repo name.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_with_repo(d: *mut Dep, repo: *const c_char) -> *mut Dep {
    ffi_catch_panic! {
        let dep = try_ref_from_ptr!(d);
        let repo = try_str_from_ptr!(repo);
        if let Cow::Owned(d) = unwrap_or_panic!(dep.with_repo(repo)) {
            Box::into_raw(Box::new(d))
        } else {
            d
        }
    }
}

/// Parse a string into an unversioned package dependency.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_new_cpn(s: *const c_char) -> *mut Dep {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let dep = unwrap_or_panic!(Dep::new_cpn(s));
        Box::into_raw(Box::new(dep))
    }
}

/// Parse a string into a Blocker's raw value.
///
/// Returns a value of 0 for nonexistence.
///
/// # Safety
/// The argument must be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_blocker_from_str(s: *const c_char) -> u32 {
    let s = try_str_from_ptr!(s);
    s.parse::<Blocker>().map(|x| x as u32).unwrap_or_default()
}

/// Return the string for a Blocker.
#[no_mangle]
pub extern "C" fn pkgcraft_dep_blocker_str(b: Blocker) -> *mut c_char {
    try_ptr_from_str!(b.as_ref())
}

/// Parse a string into a SlotOperator's raw value.
///
/// Returns a value of 0 for nonexistence.
///
/// # Safety
/// The argument must be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_slot_op_from_str(s: *const c_char) -> u32 {
    let s = try_str_from_ptr!(s);
    s.parse::<SlotOperator>()
        .map(|x| x as u32)
        .unwrap_or_default()
}

/// Return the string for a SlotOperator.
#[no_mangle]
pub extern "C" fn pkgcraft_dep_slot_op_str(op: SlotOperator) -> *mut c_char {
    try_ptr_from_str!(op.as_ref())
}

/// Compare two package dependencies returning -1, 0, or 1 if the first is less than, equal to, or
/// greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Dep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_cmp(d1: *mut Dep, d2: *mut Dep) -> c_int {
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);

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
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);
    d1.intersects(d2)
}

/// Determine if a package dependency intersects with a Cpv.
///
/// # Safety
/// The arguments must be non-null Cpv and Dep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_intersects_cpv(d: *mut Dep, c: *mut Cpv) -> bool {
    let d = try_ref_from_ptr!(d);
    let c = try_ref_from_ptr!(c);
    d.intersects(c)
}

/// Get the category of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "cat".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_category(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    try_ptr_from_str!(dep.category())
}

/// Get the package name of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_package(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    try_ptr_from_str!(dep.package())
}

/// Get a package dependency's raw blocker value.
/// For example, the package dependency "!cat/pkg" has a weak blocker.
///
/// Returns a value of 0 for nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_blocker(d: *mut Dep) -> u32 {
    let dep = try_ref_from_ptr!(d);
    dep.blocker().map(|x| x as u32).unwrap_or_default()
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
    let dep = try_ref_from_ptr!(d);
    match dep.version() {
        Some(v) => Box::into_raw(Box::new(v.clone())),
        None => ptr::null_mut(),
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
    let dep = try_ref_from_ptr!(d);
    match dep.slot() {
        Some(s) => try_ptr_from_str!(s),
        None => ptr::null_mut(),
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
    let dep = try_ref_from_ptr!(d);
    match dep.subslot() {
        Some(s) => try_ptr_from_str!(s),
        None => ptr::null_mut(),
    }
}

/// Get a package dependency's raw slot operator value.
/// For example, the package dependency "=cat/pkg-1-r2:0=" has an equal slot operator.
///
/// Returns a value of 0 for nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_slot_op(d: *mut Dep) -> u32 {
    let dep = try_ref_from_ptr!(d);
    dep.slot_op().map(|x| x as u32).unwrap_or_default()
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
    let dep = try_ref_from_ptr!(d);
    match dep.use_deps() {
        Some(use_deps) => iter_to_array!(use_deps.iter(), len, str_to_raw),
        None => ptr::null_mut(),
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
    let dep = try_ref_from_ptr!(d);
    match dep.repo() {
        Some(s) => try_ptr_from_str!(s),
        None => ptr::null_mut(),
    }
}

/// Get the package and revision of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_p(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    try_ptr_from_str!(dep.p())
}

/// Get the package, version, and revision of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg-1-r2".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_pf(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    try_ptr_from_str!(dep.pf())
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
    let dep = try_ref_from_ptr!(d);
    match dep.pr().as_str() {
        "" => ptr::null_mut(),
        s => try_ptr_from_str!(s),
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
    let dep = try_ref_from_ptr!(d);
    match dep.pv().as_str() {
        "" => ptr::null_mut(),
        s => try_ptr_from_str!(s),
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
    let dep = try_ref_from_ptr!(d);
    match dep.pvr().as_str() {
        "" => ptr::null_mut(),
        s => try_ptr_from_str!(s),
    }
}

/// Get the category and package of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_cpn(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    try_ptr_from_str!(dep.cpn())
}

/// Get the category, package, and version of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "cat/pkg-1-r2".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_cpv(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    try_ptr_from_str!(dep.cpv())
}

/// Return the string for a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_str(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    try_ptr_from_str!(dep.to_string())
}

/// Return the hash value for a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_hash(d: *mut Dep) -> u64 {
    let dep = try_ref_from_ptr!(d);
    hash(dep)
}

/// Return the restriction for a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_restrict(d: *mut Dep) -> *mut Restrict {
    let dep = try_ref_from_ptr!(d);
    Box::into_raw(Box::new(dep.into()))
}

/// Determine if a restriction matches a package dependency.
///
/// # Safety
/// The arguments must be valid Restrict and Dep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_restrict_matches(d: *mut Dep, r: *mut Restrict) -> bool {
    let dep = try_ref_from_ptr!(d);
    let restrict = try_ref_from_ptr!(r);
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
