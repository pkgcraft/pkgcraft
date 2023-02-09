use std::cmp::Ordering;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::{fmt, ptr};

use pkgcraft::atom::Atom;
use pkgcraft::depset::{self, IntoIteratorDepSet, Uri};
use pkgcraft::eapi::{Eapi, IntoEapi};
use pkgcraft::set::Ordered;
use pkgcraft::utils::hash;

use crate::macros::*;

/// DepSet flattened unit variants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum DepSetUnit {
    Atom,
    String,
    Uri,
}

/// DepSet variants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum DepSetKind {
    PkgDep,
    Restrict,
    RequiredUse,
    Properties,
    SrcUri,
    License,
}

/// Opaque wrapper for DepSet objects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DepSetW {
    Atom(depset::DepSet<Atom>),
    String(depset::DepSet<String>),
    Uri(depset::DepSet<Uri>),
}

impl fmt::Display for DepSetW {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Atom(d) => write!(f, "{d}"),
            Self::String(d) => write!(f, "{d}"),
            Self::Uri(d) => write!(f, "{d}"),
        }
    }
}

/// C-compatible wrapper for DepSet objects.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DepSet {
    unit: DepSetUnit,
    kind: DepSetKind,
    dep: *mut DepSetW,
}

impl Drop for DepSet {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.dep));
        }
    }
}

impl DepSet {
    pub(crate) fn new_atom(d: depset::DepSet<Atom>) -> Self {
        Self {
            unit: DepSetUnit::Atom,
            kind: DepSetKind::PkgDep,
            dep: Box::into_raw(Box::new(DepSetW::Atom(d))),
        }
    }

    pub(crate) fn new_string(d: depset::DepSet<String>, kind: DepSetKind) -> Self {
        Self {
            unit: DepSetUnit::String,
            kind,
            dep: Box::into_raw(Box::new(DepSetW::String(d))),
        }
    }

    pub(crate) fn new_uri(d: depset::DepSet<Uri>) -> Self {
        Self {
            unit: DepSetUnit::Uri,
            kind: DepSetKind::SrcUri,
            dep: Box::into_raw(Box::new(DepSetW::Uri(d))),
        }
    }
}

impl Hash for DepSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl PartialEq for DepSet {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl Eq for DepSet {}

impl Deref for DepSet {
    type Target = DepSetW;

    fn deref(&self) -> &Self::Target {
        null_ptr_check!(self.dep.as_ref())
    }
}

impl fmt::Display for DepSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

/// Opaque wrapper for DepSet iterators.
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
            Self::Atom(iter) => iter.next().map(DepRestrict::new_atom),
            Self::String(iter) => iter.next().map(DepRestrict::new_string),
            Self::Uri(iter) => iter.next().map(DepRestrict::new_uri),
        }
    }
}

/// Opaque wrapper for DepRestrict objects.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DepRestrictW {
    Atom(depset::DepRestrict<Atom>),
    String(depset::DepRestrict<String>),
    Uri(depset::DepRestrict<Uri>),
}

impl fmt::Display for DepRestrictW {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Atom(d) => write!(f, "{d}"),
            Self::String(d) => write!(f, "{d}"),
            Self::Uri(d) => write!(f, "{d}"),
        }
    }
}

/// DepRestrict variants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum DepKind {
    Enabled,
    Disabled,
    AllOf,
    AnyOf,
    ExactlyOneOf,
    AtMostOneOf,
    UseEnabled,
    UseDisabled,
}

impl<T: Ordered> From<&depset::DepRestrict<T>> for DepKind {
    fn from(d: &depset::DepRestrict<T>) -> Self {
        use depset::DepRestrict::*;
        match d {
            Enabled(_) => Self::Enabled,
            Disabled(_) => Self::Disabled,
            AllOf(_) => Self::AllOf,
            AnyOf(_) => Self::AnyOf,
            ExactlyOneOf(_) => Self::ExactlyOneOf,
            AtMostOneOf(_) => Self::AtMostOneOf,
            UseEnabled(_, _) => Self::UseEnabled,
            UseDisabled(_, _) => Self::UseDisabled,
        }
    }
}

/// C-compatible wrapper for DepRestrict objects.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DepRestrict {
    unit: DepSetUnit,
    kind: DepKind,
    dep: *mut DepRestrictW,
}

impl Drop for DepRestrict {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.dep));
        }
    }
}

impl DepRestrict {
    pub(crate) fn new_atom(d: depset::DepRestrict<Atom>) -> Self {
        Self {
            unit: DepSetUnit::Atom,
            kind: DepKind::from(&d),
            dep: Box::into_raw(Box::new(DepRestrictW::Atom(d))),
        }
    }

    pub(crate) fn new_string(d: depset::DepRestrict<String>) -> Self {
        Self {
            unit: DepSetUnit::String,
            kind: DepKind::from(&d),
            dep: Box::into_raw(Box::new(DepRestrictW::String(d))),
        }
    }

    pub(crate) fn new_uri(d: depset::DepRestrict<Uri>) -> Self {
        Self {
            unit: DepSetUnit::Uri,
            kind: DepKind::from(&d),
            dep: Box::into_raw(Box::new(DepRestrictW::Uri(d))),
        }
    }
}

impl Hash for DepRestrict {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl Ord for DepRestrict {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deref().cmp(other.deref())
    }
}

impl PartialOrd for DepRestrict {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for DepRestrict {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for DepRestrict {}

impl Deref for DepRestrict {
    type Target = DepRestrictW;

    fn deref(&self) -> &Self::Target {
        null_ptr_check!(self.dep.as_ref())
    }
}

impl fmt::Display for DepRestrict {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

/// Opaque wrapper for flattened DepSet iterators.
#[derive(Debug)]
pub enum DepSetIntoIterFlatten {
    Atom(depset::DepSetIntoIterFlatten<Atom>),
    String(depset::DepSetIntoIterFlatten<String>),
    Uri(depset::DepSetIntoIterFlatten<Uri>),
}

impl Iterator for DepSetIntoIterFlatten {
    type Item = *mut c_void;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Atom(iter) => iter
                .next()
                .map(|x| Box::into_raw(Box::new(x)) as *mut c_void),
            Self::String(iter) => iter
                .next()
                .map(|x| CString::new(x.as_str()).unwrap().into_raw() as *mut c_void),
            Self::Uri(iter) => iter
                .next()
                .map(|x| Box::into_raw(Box::new(x)) as *mut c_void),
        }
    }
}

/// Opaque wrapper for recursive DepSet iterators.
#[derive(Debug)]
pub enum DepSetIntoIterRecursive {
    Atom(depset::DepSetIntoIterRecursive<Atom>),
    String(depset::DepSetIntoIterRecursive<String>),
    Uri(depset::DepSetIntoIterRecursive<Uri>),
}

impl Iterator for DepSetIntoIterRecursive {
    type Item = DepRestrict;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Atom(iter) => iter.next().map(DepRestrict::new_atom),
            Self::String(iter) => iter.next().map(DepRestrict::new_string),
            Self::Uri(iter) => iter.next().map(DepRestrict::new_uri),
        }
    }
}

/// Parse a string into a PkgDep DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_pkg_dep(
    s: *const c_char,
    eapi: *const Eapi,
) -> *mut DepSet {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let eapi = unwrap_or_return!(IntoEapi::into_eapi(eapi), ptr::null_mut());
    let opt_dep = unwrap_or_return!(depset::pkg_dep(s, eapi), ptr::null_mut());
    let dep = DepSet::new_atom(opt_dep.unwrap_or_default());
    Box::into_raw(Box::new(dep))
}

/// Parse a string into a Restrict DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_restrict(s: *const c_char) -> *mut DepSet {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let opt_dep = unwrap_or_return!(depset::restrict(s), ptr::null_mut());
    let dep = DepSet::new_string(opt_dep.unwrap_or_default(), DepSetKind::Restrict);
    Box::into_raw(Box::new(dep))
}

/// Parse a string into a RequiredUse DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_required_use(
    s: *const c_char,
    eapi: *const Eapi,
) -> *mut DepSet {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let eapi = unwrap_or_return!(IntoEapi::into_eapi(eapi), ptr::null_mut());
    let opt_dep = unwrap_or_return!(depset::required_use(s, eapi), ptr::null_mut());
    let dep = DepSet::new_string(opt_dep.unwrap_or_default(), DepSetKind::RequiredUse);
    Box::into_raw(Box::new(dep))
}

/// Parse a string into a Properties DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_properties(s: *const c_char) -> *mut DepSet {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let opt_dep = unwrap_or_return!(depset::properties(s), ptr::null_mut());
    let dep = DepSet::new_string(opt_dep.unwrap_or_default(), DepSetKind::Properties);
    Box::into_raw(Box::new(dep))
}

/// Parse a string into a SrcUri DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_src_uri(
    s: *const c_char,
    eapi: *const Eapi,
) -> *mut DepSet {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let eapi = unwrap_or_return!(IntoEapi::into_eapi(eapi), ptr::null_mut());
    let opt_dep = unwrap_or_return!(depset::src_uri(s, eapi), ptr::null_mut());
    let dep = DepSet::new_uri(opt_dep.unwrap_or_default());
    Box::into_raw(Box::new(dep))
}

/// Parse a string into a License DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_license(s: *const c_char) -> *mut DepSet {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let opt_dep = unwrap_or_return!(depset::license(s), ptr::null_mut());
    let dep = DepSet::new_string(opt_dep.unwrap_or_default(), DepSetKind::License);
    Box::into_raw(Box::new(dep))
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
pub unsafe extern "C" fn pkgcraft_depset_into_iter(d: *mut DepSet) -> *mut DepSetIntoIter {
    let deps = null_ptr_check!(d.as_ref());
    let iter = match deps.deref().clone() {
        DepSetW::Atom(d) => DepSetIntoIter::Atom(d.into_iter()),
        DepSetW::String(d) => DepSetIntoIter::String(d.into_iter()),
        DepSetW::Uri(d) => DepSetIntoIter::Uri(d.into_iter()),
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
pub unsafe extern "C" fn pkgcraft_depset_into_iter_next(
    i: *mut DepSetIntoIter,
) -> *mut DepRestrict {
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
pub unsafe extern "C" fn pkgcraft_depset_into_iter_free(i: *mut DepSetIntoIter) {
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

/// Return a flattened iterator for a DepRestrict.
///
/// # Safety
/// The argument must be a non-null DepRestrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_into_iter_flatten(
    d: *mut DepRestrict,
) -> *mut DepSetIntoIterFlatten {
    let dep = null_ptr_check!(d.as_ref());
    let iter = match dep.deref().clone() {
        DepRestrictW::Atom(d) => DepSetIntoIterFlatten::Atom(d.into_iter_flatten()),
        DepRestrictW::String(d) => DepSetIntoIterFlatten::String(d.into_iter_flatten()),
        DepRestrictW::Uri(d) => DepSetIntoIterFlatten::Uri(d.into_iter_flatten()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return a flattened iterator for a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_into_iter_flatten(
    d: *mut DepSet,
) -> *mut DepSetIntoIterFlatten {
    let deps = null_ptr_check!(d.as_ref());
    let iter = match deps.deref().clone() {
        DepSetW::Atom(d) => DepSetIntoIterFlatten::Atom(d.into_iter_flatten()),
        DepSetW::String(d) => DepSetIntoIterFlatten::String(d.into_iter_flatten()),
        DepSetW::Uri(d) => DepSetIntoIterFlatten::Uri(d.into_iter_flatten()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a flattened depset iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSetIntoIterFlatten pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_into_iter_flatten_next(
    i: *mut DepSetIntoIterFlatten,
) -> *mut c_void {
    let iter = null_ptr_check!(i.as_mut());
    iter.next().unwrap_or(ptr::null_mut())
}

/// Free a flattened depset iterator.
///
/// # Safety
/// The argument must be a non-null DepSetIntoIterFlatten pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_into_iter_flatten_free(i: *mut DepSetIntoIterFlatten) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Return a recursive iterator for a DepRestrict.
///
/// # Safety
/// The argument must be a non-null DepRestrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_into_iter_recursive(
    d: *mut DepRestrict,
) -> *mut DepSetIntoIterRecursive {
    let dep = null_ptr_check!(d.as_ref());
    let iter = match dep.deref().clone() {
        DepRestrictW::Atom(d) => DepSetIntoIterRecursive::Atom(d.into_iter_recursive()),
        DepRestrictW::String(d) => DepSetIntoIterRecursive::String(d.into_iter_recursive()),
        DepRestrictW::Uri(d) => DepSetIntoIterRecursive::Uri(d.into_iter_recursive()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return a recursive iterator for a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_into_iter_recursive(
    d: *mut DepSet,
) -> *mut DepSetIntoIterRecursive {
    let deps = null_ptr_check!(d.as_ref());
    let iter = match deps.deref().clone() {
        DepSetW::Atom(d) => DepSetIntoIterRecursive::Atom(d.into_iter_recursive()),
        DepSetW::String(d) => DepSetIntoIterRecursive::String(d.into_iter_recursive()),
        DepSetW::Uri(d) => DepSetIntoIterRecursive::Uri(d.into_iter_recursive()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a recursive depset iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSetIntoIterRecursive pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_into_iter_recursive_next(
    i: *mut DepSetIntoIterRecursive,
) -> *mut DepRestrict {
    let iter = null_ptr_check!(i.as_mut());
    iter.next()
        .map(|x| Box::into_raw(Box::new(x)))
        .unwrap_or(ptr::null_mut())
}

/// Free a recursive depset iterator.
///
/// # Safety
/// The argument must be a non-null DepSetIntoIterFlatten pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_into_iter_recursive_free(i: *mut DepSetIntoIterRecursive) {
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
