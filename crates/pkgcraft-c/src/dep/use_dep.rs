use std::cmp::Ordering;
use std::ffi::{CString, c_char, c_int};
use std::ops::Deref;
use std::ptr;

use pkgcraft::dep;
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::utils::boxed;

/// Opaque wrapper for pkgcraft::dep::UseDep.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UseDepWrapper(dep::UseDep);

/// C-compatible wrapper for pkgcraft::dep::UseDep.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct UseDep {
    flag: *mut c_char,
    kind: dep::UseDepKind,
    enabled: bool,
    // underscore suffix to avoid reserved keyword with cython bindings
    default_: *mut bool,
    dep: *mut UseDepWrapper,
}

impl From<dep::UseDep> for UseDep {
    fn from(u: dep::UseDep) -> Self {
        UseDep {
            kind: u.kind(),
            enabled: u.enabled(),
            flag: try_ptr_from_str!(u.flag()),
            default_: u.default().map(boxed).unwrap_or(ptr::null_mut()),
            dep: boxed(UseDepWrapper(u)),
        }
    }
}

impl Deref for UseDep {
    type Target = dep::UseDep;

    fn deref(&self) -> &Self::Target {
        let wrapper = try_ref_from_ptr!(self.dep);
        &wrapper.0
    }
}

impl Drop for UseDep {
    fn drop(&mut self) {
        unsafe {
            drop(CString::from_raw(self.flag));
            if !self.default_.is_null() {
                drop(Box::from_raw(self.default_));
            }
            drop(Box::from_raw(self.dep));
        }
    }
}

/// Parse a string into a package USE dependency.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_use_dep_new(s: *const c_char) -> *mut UseDep {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let use_dep = unwrap_or_panic!(dep::UseDep::try_new(s).map(|u| u.into()));
        Box::into_raw(Box::new(use_dep))
    }
}

/// Compare two package USE dependencies returning -1, 0, or 1 if the first is less than, equal to,
/// or greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null UseDep pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_use_dep_cmp(u1: *mut UseDep, u2: *mut UseDep) -> c_int {
    let u1 = try_ref_from_ptr!(u1);
    let u2 = try_ref_from_ptr!(u2);

    match u1.cmp(u2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the hash value for a package USE dependency.
///
/// # Safety
/// The argument must be a non-null UseDep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_use_dep_hash(u: *mut UseDep) -> u64 {
    let use_dep = try_ref_from_ptr!(u);
    hash(use_dep.deref())
}

/// Return the string for a package USE dependency.
///
/// # Safety
/// The argument must be a non-null UseDep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_use_dep_str(u: *mut UseDep) -> *mut c_char {
    let u = try_ref_from_ptr!(u);
    try_ptr_from_str!(u.to_string())
}

/// Free a package USE dependency.
///
/// # Safety
/// The argument must be a UseDep pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_use_dep_free(u: *mut UseDep) {
    if !u.is_null() {
        unsafe { drop(Box::from_raw(u)) };
    }
}
