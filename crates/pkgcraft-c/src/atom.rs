use std::cmp::Ordering;
use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;
use std::str::FromStr;

use pkgcraft::atom::{self, Atom, Blocker, SlotOperator};
use pkgcraft::eapi::{Eapi, IntoEapi};
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::types::AtomVersion;
use crate::utils::str_to_raw;

pub mod version;

/// Parse a string into an atom using a specific EAPI. Pass NULL for the eapi argument in
/// order to parse using the latest EAPI with extensions (e.g. support for repo deps).
///
/// Returns NULL on error.
///
/// # Safety
/// The atom argument should be a UTF-8 string while eapi may be NULL to use the default EAPI.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_new(atom: *const c_char, eapi: *const Eapi) -> *mut Atom {
    let atom = null_ptr_check!(atom.as_ref());
    let atom = unsafe { unwrap_or_return!(CStr::from_ptr(atom).to_str(), ptr::null_mut()) };
    let eapi = unwrap_or_return!(IntoEapi::into_eapi(eapi), ptr::null_mut());
    let atom = unwrap_or_return!(Atom::new(atom, eapi), ptr::null_mut());
    Box::into_raw(Box::new(atom))
}

/// Parse a CPV string into an atom.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_cpv_new(s: *const c_char) -> *mut Atom {
    let s = null_ptr_check!(s.as_ref());
    let atom = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let atom = unwrap_or_return!(atom::cpv(atom), ptr::null_mut());
    Box::into_raw(Box::new(atom))
}

/// Parse a string into a Blocker.
///
/// Returns -1 on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_blocker_from_str(s: *const c_char) -> c_int {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), -1) };
    Blocker::from_str(s).map(|b| b as c_int).unwrap_or(-1)
}

/// Parse a string into a SlotOperator.
///
/// Returns -1 on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_slot_op_from_str(s: *const c_char) -> c_int {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), -1) };
    SlotOperator::from_str(s).map(|b| b as c_int).unwrap_or(-1)
}

/// Compare two atoms returning -1, 0, or 1 if the first atom is less than, equal to, or greater
/// than the second atom, respectively.
///
/// # Safety
/// The arguments must be non-null Atom pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_cmp(a1: *mut Atom, a2: *mut Atom) -> c_int {
    let a1 = null_ptr_check!(a1.as_ref());
    let a2 = null_ptr_check!(a2.as_ref());

    match a1.cmp(a2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Determine if two atoms intersect.
///
/// # Safety
/// The arguments must be non-null Atom pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_intersects(a1: *mut Atom, a2: *mut Atom) -> bool {
    let a1 = null_ptr_check!(a1.as_ref());
    let a2 = null_ptr_check!(a2.as_ref());
    a1.intersects(a2)
}

/// Return an atom's category, e.g. the atom "=cat/pkg-1-r2" has a category of "cat".
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_category(atom: *mut Atom) -> *mut c_char {
    let atom = null_ptr_check!(atom.as_ref());
    CString::new(atom.category()).unwrap().into_raw()
}

/// Return an atom's package, e.g. the atom "=cat/pkg-1-r2" has a package of "pkg".
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_package(atom: *mut Atom) -> *mut c_char {
    let atom = null_ptr_check!(atom.as_ref());
    CString::new(atom.package()).unwrap().into_raw()
}

/// Return an atom's blocker status, e.g. the atom "!cat/pkg" has a weak blocker.
///
/// Returns -1 on nonexistence.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_blocker(atom: *mut Atom) -> c_int {
    let atom = null_ptr_check!(atom.as_ref());
    atom.blocker().map(|b| b as c_int).unwrap_or(-1)
}

/// Return an atom's version, e.g. the atom "=cat/pkg-1-r2" has a version of "1-r2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_version(atom: *mut Atom) -> *mut AtomVersion {
    let atom = null_ptr_check!(atom.as_ref());
    match atom.version() {
        None => ptr::null_mut(),
        Some(v) => Box::into_raw(Box::new(v.clone())),
    }
}

/// Return an atom's revision, e.g. the atom "=cat/pkg-1-r2" has a revision of "2".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_revision(atom: *mut Atom) -> *mut c_char {
    let atom = null_ptr_check!(atom.as_ref());
    match atom.revision() {
        None => ptr::null_mut(),
        Some(r) => CString::new(r.as_str()).unwrap().into_raw(),
    }
}

/// Return an atom's slot, e.g. the atom "=cat/pkg-1-r2:3" has a slot of "3".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_slot(atom: *mut Atom) -> *mut c_char {
    let atom = null_ptr_check!(atom.as_ref());
    match atom.slot() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Return an atom's subslot, e.g. the atom "=cat/pkg-1-r2:3/4" has a subslot of "4".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_subslot(atom: *mut Atom) -> *mut c_char {
    let atom = null_ptr_check!(atom.as_ref());
    match atom.subslot() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Return an atom's slot operator, e.g. the atom "=cat/pkg-1-r2:0=" has an equal slot
/// operator.
///
/// Returns -1 on nonexistence.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_slot_op(atom: *mut Atom) -> c_int {
    let atom = null_ptr_check!(atom.as_ref());
    atom.slot_op().map(|op| op as c_int).unwrap_or(-1)
}

/// Return an atom's USE dependencies, e.g. the atom "=cat/pkg-1-r2[a,b,c]" has USE
/// dependencies of "a, b, c".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_use_deps(
    atom: *mut Atom,
    len: *mut usize,
) -> *mut *mut c_char {
    // TODO: switch from usize to std::os::raw::c_size_t when it's stable.
    let atom = null_ptr_check!(atom.as_ref());
    match atom.use_deps() {
        None => ptr::null_mut(),
        Some(use_deps) => iter_to_array!(use_deps.iter(), len, str_to_raw),
    }
}

/// Return an atom's repo, e.g. the atom "=cat/pkg-1-r2:3/4::repo" has a repo of "repo".
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_repo(atom: *mut Atom) -> *mut c_char {
    let atom = null_ptr_check!(atom.as_ref());
    match atom.repo() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Return an atom's CPN, e.g. the atom "=cat/pkg-1-r2" has a CPN of "cat/pkg".
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_cpn(atom: *mut Atom) -> *mut c_char {
    let atom = null_ptr_check!(atom.as_ref());
    CString::new(atom.cpn()).unwrap().into_raw()
}

/// Return an atom's CPV, e.g. the atom "=cat/pkg-1-r2" has a CPV of "cat/pkg-1-r2".
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_cpv(atom: *mut Atom) -> *mut c_char {
    let atom = null_ptr_check!(atom.as_ref());
    CString::new(atom.cpv()).unwrap().into_raw()
}

/// Return the string for an atom.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_str(atom: *mut Atom) -> *mut c_char {
    let atom = null_ptr_check!(atom.as_ref());
    CString::new(atom.to_string()).unwrap().into_raw()
}

/// Return the hash value for an atom.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_hash(atom: *mut Atom) -> u64 {
    let atom = null_ptr_check!(atom.as_ref());
    hash(atom)
}

/// Return the restriction for an atom.
///
/// # Safety
/// The argument must be a non-null Atom pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_restrict(atom: *mut Atom) -> *mut Restrict {
    let atom = null_ptr_check!(atom.as_ref());
    Box::into_raw(Box::new(atom.into()))
}

/// Determine if a restriction matches an atom.
///
/// # Safety
/// The arguments must be valid Restrict and Atom pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_restrict_matches(atom: *mut Atom, r: *mut Restrict) -> bool {
    let atom = null_ptr_check!(atom.as_ref());
    let r = null_ptr_check!(r.as_ref());
    r.matches(atom)
}

/// Free an atom.
///
/// # Safety
/// The argument must be a Atom pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_atom_free(atom: *mut Atom) {
    if !atom.is_null() {
        unsafe { drop(Box::from_raw(atom)) };
    }
}
