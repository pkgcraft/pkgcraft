use std::borrow::Cow;
use std::cmp::Ordering;
use std::ffi::{c_char, c_int};
use std::{ptr, slice};

use pkgcraft::dep::{Blocker, Cpn, Cpv, Dep, DepField, SlotOperator, Version};
use pkgcraft::eapi::Eapi;
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::traits::Intersects;
use pkgcraft::utils::hash;

use crate::eapi::eapi_or_default;
use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::utils::{boxed, obj_to_str};

use super::use_dep::UseDep;

/// Parse a string into a package dependency using a specific EAPI. Pass NULL for the eapi argument
/// in order to parse using the latest EAPI with extensions (e.g. support for repo deps).
///
/// Returns NULL on error.
///
/// # Safety
/// The eapi argument may be NULL to use the default EAPI.
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_parse(
    s: *const c_char,
    eapi: *const Eapi,
) -> *const c_char {
    ffi_catch_panic! {
        let val = try_str_from_ptr!(s);
        let eapi = option_from_ptr!(eapi).unwrap_or_default();
        unwrap_or_panic!(eapi.dep(val));
        s
    }
}

/// Return a package dependency without the specified fields.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must a valid Dep pointer and DepField values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_without(
    d: *mut Dep,
    fields: *mut DepField,
    len: usize,
) -> *mut Dep {
    ffi_catch_panic! {
        let dep = try_ref_from_ptr!(d);
        let fields = unsafe { slice::from_raw_parts(fields, len) };
        let dep = dep.without(fields.iter().copied());

        if let Cow::Owned(d) = unwrap_or_panic!(dep) {
            Box::into_raw(Box::new(d))
        } else {
            d
        }
    }
}

/// Return a package dependency without optional fields.
///
/// # Safety
/// The argument must a valid Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_unversioned(d: *mut Dep) -> *mut Dep {
    let dep = try_ref_from_ptr!(d);
    if let Cow::Owned(d) = dep.unversioned() {
        Box::into_raw(Box::new(d))
    } else {
        d
    }
}

/// Return a package dependency without optional fields except version.
///
/// # Safety
/// The argument must a valid Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_versioned(d: *mut Dep) -> *mut Dep {
    let dep = try_ref_from_ptr!(d);
    if let Cow::Owned(d) = dep.versioned() {
        Box::into_raw(Box::new(d))
    } else {
        d
    }
}

/// Return a package dependency without USE dependencies.
///
/// # Safety
/// The argument must a valid Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_no_use_deps(d: *mut Dep) -> *mut Dep {
    let dep = try_ref_from_ptr!(d);
    if let Cow::Owned(d) = dep.no_use_deps() {
        Box::into_raw(Box::new(d))
    } else {
        d
    }
}

/// Return a package dependency modifying the specified fields with corresponding string values.
/// Use null pointers for string values to unset a given field.
///
/// Returns NULL on error.
///
/// # Safety
/// The fields and values arguments must be equal length arrays of DepFields with
/// corresponding string values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_modify(
    d: *mut Dep,
    fields: *mut DepField,
    values: *mut *mut c_char,
    len: usize,
) -> *mut Dep {
    ffi_catch_panic! {
        let dep = try_ref_from_ptr!(d);
        let fields = unsafe { slice::from_raw_parts(fields, len) };
        let values = unsafe { slice::from_raw_parts(values, len) };
        let iterable = fields.iter().zip(values.iter())
            .map(|(f, p)| (*f, option_from_ptr!(p).map(|_| try_str_from_ptr!(p))));

        if let Cow::Owned(d) = unwrap_or_panic!(dep.modify(iterable)) {
            Box::into_raw(Box::new(d))
        } else {
            d
        }
    }
}

/// Parse a string into a Blocker's raw value.
///
/// Returns a value of 0 for nonexistence.
///
/// # Safety
/// The argument must be a UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_blocker_from_str(s: *const c_char) -> u32 {
    let s = try_str_from_ptr!(s);
    s.parse::<Blocker>().map(|x| x as u32).unwrap_or_default()
}

/// Return the string for a Blocker.
#[unsafe(no_mangle)]
pub extern "C" fn pkgcraft_dep_blocker_str(b: Blocker) -> *mut c_char {
    try_ptr_from_str!(b.as_ref())
}

/// Parse a string into a SlotOperator's raw value.
///
/// Returns a value of 0 for nonexistence.
///
/// # Safety
/// The argument must be a UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_slot_op_from_str(s: *const c_char) -> u32 {
    let s = try_str_from_ptr!(s);
    s.parse::<SlotOperator>()
        .map(|x| x as u32)
        .unwrap_or_default()
}

/// Return the string for a SlotOperator.
#[unsafe(no_mangle)]
pub extern "C" fn pkgcraft_dep_slot_op_str(op: SlotOperator) -> *mut c_char {
    try_ptr_from_str!(op.as_ref())
}

/// Compare two package dependencies returning -1, 0, or 1 if the first is less than, equal to, or
/// greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Dep pointers.
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_intersects(d1: *mut Dep, d2: *mut Dep) -> bool {
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);
    d1.intersects(d2)
}

/// Determine if a package dependency intersects with a Cpv.
///
/// # Safety
/// The arguments must be non-null Cpv and Dep pointers.
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_category(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    try_ptr_from_str!(dep.category())
}

/// Get the package name of a package dependency.
/// For example, the package dependency "=cat/pkg-1-r2" returns "pkg".
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_use_deps(
    d: *mut Dep,
    len: *mut usize,
) -> *mut *mut UseDep {
    // TODO: switch from usize to std::os::raw::c_size_t when it's stable.
    let dep = try_ref_from_ptr!(d);
    match dep.use_deps() {
        Some(use_deps) => {
            iter_to_array!(use_deps.iter(), len, |u| boxed(u.clone().into()))
        }
        None => ptr::null_mut(),
    }
}

/// Get the USE dependencies of a package dependency as raw strings.
/// For example, the package dependency "=cat/pkg-1-r2[a,b,c]" has USE dependencies of "a, b, c".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_use_deps_str(
    d: *mut Dep,
    len: *mut usize,
) -> *mut *mut c_char {
    // TODO: switch from usize to std::os::raw::c_size_t when it's stable.
    let dep = try_ref_from_ptr!(d);
    match dep.use_deps() {
        Some(use_deps) => iter_to_array!(use_deps.iter(), len, obj_to_str),
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_repo(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    match dep.repo() {
        Some(s) => try_ptr_from_str!(s),
        None => ptr::null_mut(),
    }
}

/// Get the Cpn of a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_cpn(d: *mut Dep) -> *mut Cpn {
    let dep = try_ref_from_ptr!(d);
    Box::into_raw(Box::new(dep.cpn().clone()))
}

/// Get the Cpv of a package dependency if one exists.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_cpv(d: *mut Dep) -> *mut Cpv {
    let dep = try_ref_from_ptr!(d);
    match dep.cpv() {
        Some(cpv) => Box::into_raw(Box::new(cpv)),
        None => ptr::null_mut(),
    }
}

/// Return the string for a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_str(d: *mut Dep) -> *mut c_char {
    let dep = try_ref_from_ptr!(d);
    try_ptr_from_str!(dep.to_string())
}

/// Return the hash value for a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_hash(d: *mut Dep) -> u64 {
    let dep = try_ref_from_ptr!(d);
    hash(dep)
}

/// Return the restriction for a package dependency.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_restrict(d: *mut Dep) -> *mut Restrict {
    let dep = try_ref_from_ptr!(d);
    Box::into_raw(Box::new(dep.into()))
}

/// Determine if a restriction matches a package dependency.
///
/// # Safety
/// The arguments must be valid Restrict and Dep pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_restrict_matches(d: *mut Dep, r: *mut Restrict) -> bool {
    let dep = try_ref_from_ptr!(d);
    let restrict = try_ref_from_ptr!(r);
    restrict.matches(dep)
}

/// Free a package dependency.
///
/// # Safety
/// The argument must be a Dep pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dep_free(d: *mut Dep) {
    if !d.is_null() {
        unsafe { drop(Box::from_raw(d)) };
    }
}
