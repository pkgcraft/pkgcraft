use std::cmp::Ordering;
use std::ffi::{c_char, c_int, c_void, CString};
use std::{fmt, ptr};

use pkgcraft::atom::Atom;
use pkgcraft::depset::{self, Uri};
use pkgcraft::utils::hash;

use crate::macros::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DepSet {
    Atom(depset::DepSet<Atom>),
    String(depset::DepSet<String>),
    Uri(depset::DepSet<Uri>),
}

impl fmt::Display for DepSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Atom(d) => write!(f, "{}", d),
            Self::String(d) => write!(f, "{}", d),
            Self::Uri(d) => write!(f, "{}", d),
        }
    }
}

#[derive(Debug)]
pub enum DepSetIntoIter {
    Atom(depset::DepSetIntoIter<Atom>),
    String(depset::DepSetIntoIter<String>),
    Uri(depset::DepSetIntoIter<Uri>),
}

impl Iterator for DepSetIntoIter {
    type Item = DepRestrict;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Atom(iter) => iter.next().map(DepRestrict::Atom),
            Self::String(iter) => iter.next().map(DepRestrict::String),
            Self::Uri(iter) => iter.next().map(DepRestrict::Uri),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DepRestrict {
    Atom(depset::DepRestrict<Atom>),
    String(depset::DepRestrict<String>),
    Uri(depset::DepRestrict<Uri>),
}

impl fmt::Display for DepRestrict {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Atom(d) => write!(f, "{}", d),
            Self::String(d) => write!(f, "{}", d),
            Self::Uri(d) => write!(f, "{}", d),
        }
    }
}

#[derive(Debug)]
pub enum DepSetFlattenIter<'a> {
    Atom(depset::DepSetFlattenIter<'a, Atom>),
    String(depset::DepSetFlattenIter<'a, String>),
    Uri(depset::DepSetFlattenIter<'a, Uri>),
}

impl<'a> Iterator for DepSetFlattenIter<'a> {
    type Item = *mut c_void;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Atom(iter) => iter
                .next()
                .map(|x| Box::into_raw(Box::new(x.clone())) as *mut c_void),
            Self::String(iter) => iter
                .next()
                .map(|x| CString::new(x.as_str()).unwrap().into_raw() as *mut c_void),
            Self::Uri(iter) => iter
                .next()
                .map(|x| Box::into_raw(Box::new(x.clone())) as *mut c_void),
        }
    }
}

/// Return the formatted string for a DepSet object.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_str(d: *mut DepSet) -> *mut c_char {
    let deps = null_ptr_check!(d.as_ref());
    CString::new(deps.to_string()).unwrap().into_raw()
}

/// Determine if two DepSets are equal.
///
/// # Safety
/// The arguments must be non-null DepSet pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_eq(d1: *mut DepSet, d2: *mut DepSet) -> bool {
    let d1 = null_ptr_check!(d1.as_ref());
    let d2 = null_ptr_check!(d2.as_ref());
    d1.eq(d2)
}

/// Return the hash value for a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_hash(d: *mut DepSet) -> u64 {
    let deps = null_ptr_check!(d.as_ref());
    hash(deps)
}

/// Return an iterator for a depset.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_iter(d: *mut DepSet) -> *mut DepSetIntoIter {
    let deps = null_ptr_check!(d.as_ref());
    let iter = match deps.clone() {
        DepSet::Atom(d) => DepSetIntoIter::Atom(d.into_iter()),
        DepSet::String(d) => DepSetIntoIter::String(d.into_iter()),
        DepSet::Uri(d) => DepSetIntoIter::Uri(d.into_iter()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a depset iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSetIntoIter pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_iter_next(i: *mut DepSetIntoIter) -> *mut DepRestrict {
    let iter = null_ptr_check!(i.as_mut());
    iter.next()
        .map(|x| Box::into_raw(Box::new(x)))
        .unwrap_or(ptr::null_mut())
}

/// Free a depset iterator.
///
/// # Safety
/// The argument must be a non-null DepSetIntoIter pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_iter_free(i: *mut DepSetIntoIter) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Compare two DepRestricts returning -1, 0, or 1 if the first is less than, equal to, or greater
/// than the second, respectively.
///
/// # Safety
/// The arguments must be non-null DepRestrict pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_cmp(
    d1: *mut DepRestrict,
    d2: *mut DepRestrict,
) -> c_int {
    let d1 = null_ptr_check!(d1.as_ref());
    let d2 = null_ptr_check!(d2.as_ref());

    match d1.cmp(d2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the hash value for a DepRestrict.
///
/// # Safety
/// The argument must be a non-null DepRestrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_hash(d: *mut DepRestrict) -> u64 {
    let deps = null_ptr_check!(d.as_ref());
    hash(deps)
}

/// Free a DepRestrict object.
///
/// # Safety
/// The argument must be a DepRestrict pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_free(r: *mut DepRestrict) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}

/// Return the formatted string for a DepRestrict object.
///
/// # Safety
/// The argument must be a non-null DepRestrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_str(d: *mut DepRestrict) -> *mut c_char {
    let deps = null_ptr_check!(d.as_ref());
    CString::new(deps.to_string()).unwrap().into_raw()
}

/// Return an iterator for a flattened DepRestrict.
///
/// # Safety
/// The argument must be a non-null DepRestrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_flatten_iter(
    d: *mut DepRestrict,
) -> *mut DepSetFlattenIter<'static> {
    let deps = null_ptr_check!(d.as_ref());
    let iter = match deps {
        DepRestrict::Atom(d) => DepSetFlattenIter::Atom(d.iter_flatten()),
        DepRestrict::String(d) => DepSetFlattenIter::String(d.iter_flatten()),
        DepRestrict::Uri(d) => DepSetFlattenIter::Uri(d.iter_flatten()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return an iterator for a flattened DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_flatten_iter(
    d: *mut DepSet,
) -> *mut DepSetFlattenIter<'static> {
    let deps = null_ptr_check!(d.as_ref());
    let iter = match deps {
        DepSet::Atom(d) => DepSetFlattenIter::Atom(d.iter_flatten()),
        DepSet::String(d) => DepSetFlattenIter::String(d.iter_flatten()),
        DepSet::Uri(d) => DepSetFlattenIter::Uri(d.iter_flatten()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a flattened depset iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSetFlatten pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_flatten_iter_next(
    i: *mut DepSetFlattenIter,
) -> *mut c_void {
    let iter = null_ptr_check!(i.as_mut());
    iter.next().unwrap_or(ptr::null_mut())
}

/// Free a flattened depset iterator.
///
/// # Safety
/// The argument must be a non-null DepSetFlatten pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_flatten_iter_free(i: *mut DepSetFlattenIter) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Free a DepSet.
///
/// # Safety
/// The argument must be a DepSet pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_free(d: *mut DepSet) {
    if !d.is_null() {
        unsafe { drop(Box::from_raw(d)) };
    }
}

/// Get the main URI from a Uri object.
///
/// # Safety
/// The argument must be a Uri pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri_uri(u: *mut Uri) -> *mut c_char {
    let uri = null_ptr_check!(u.as_ref());
    CString::new(uri.uri()).unwrap().into_raw()
}

/// Get the filename rename for a Uri.
///
/// Returns NULL when no rename exists.
///
/// # Safety
/// The argument must be a Uri pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri_rename(u: *mut Uri) -> *mut c_char {
    let uri = null_ptr_check!(u.as_ref());
    match uri.rename() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
    }
}

/// Return the formatted string for a Uri object.
///
/// # Safety
/// The argument must be a Uri pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri_str(u: *mut Uri) -> *mut c_char {
    let uri = null_ptr_check!(u.as_ref());
    CString::new(uri.to_string()).unwrap().into_raw()
}

/// Free a Uri object.
///
/// # Safety
/// The argument must be a non-null Uri pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri_free(u: *mut Uri) {
    if !u.is_null() {
        unsafe { drop(Box::from_raw(u)) };
    }
}
