use std::cmp::Ordering;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::{fmt, ptr};

use pkgcraft::dep::{self, IntoIteratorDepSet, PkgDep, Uri};
use pkgcraft::eapi::{Eapi, IntoEapi};
use pkgcraft::set::Ordered;
use pkgcraft::utils::hash;

use crate::macros::*;

/// DepSet flattened unit variants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum DepUnit {
    PkgDep,
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
    PkgDep(dep::DepSet<PkgDep>),
    String(dep::DepSet<String>),
    Uri(dep::DepSet<Uri>),
}

impl fmt::Display for DepSetW {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::PkgDep(d) => write!(f, "{d}"),
            Self::String(d) => write!(f, "{d}"),
            Self::Uri(d) => write!(f, "{d}"),
        }
    }
}

/// C-compatible wrapper for DepSet objects.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DepSet {
    unit: DepUnit,
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
    pub(crate) fn new_dep(d: dep::DepSet<PkgDep>) -> Self {
        Self {
            unit: DepUnit::PkgDep,
            kind: DepSetKind::PkgDep,
            dep: Box::into_raw(Box::new(DepSetW::PkgDep(d))),
        }
    }

    pub(crate) fn new_string(d: dep::DepSet<String>, kind: DepSetKind) -> Self {
        Self {
            unit: DepUnit::String,
            kind,
            dep: Box::into_raw(Box::new(DepSetW::String(d))),
        }
    }

    pub(crate) fn new_uri(d: dep::DepSet<Uri>) -> Self {
        Self {
            unit: DepUnit::Uri,
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
    PkgDep(dep::DepSetIntoIter<PkgDep>),
    String(dep::DepSetIntoIter<String>),
    Uri(dep::DepSetIntoIter<Uri>),
}

impl Iterator for DepSetIntoIter {
    type Item = Dep;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::PkgDep(iter) => iter.next().map(Dep::new_dep),
            Self::String(iter) => iter.next().map(Dep::new_string),
            Self::Uri(iter) => iter.next().map(Dep::new_uri),
        }
    }
}

/// Opaque wrapper for Dep objects.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DepW {
    PkgDep(dep::Dep<PkgDep>),
    String(dep::Dep<String>),
    Uri(dep::Dep<Uri>),
}

impl fmt::Display for DepW {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::PkgDep(d) => write!(f, "{d}"),
            Self::String(d) => write!(f, "{d}"),
            Self::Uri(d) => write!(f, "{d}"),
        }
    }
}

/// Dep variants.
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

impl<T: Ordered> From<&dep::Dep<T>> for DepKind {
    fn from(d: &dep::Dep<T>) -> Self {
        use dep::Dep::*;
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

/// C-compatible wrapper for Dep objects.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Dep {
    unit: DepUnit,
    kind: DepKind,
    dep: *mut DepW,
}

impl Drop for Dep {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.dep));
        }
    }
}

impl Dep {
    pub(crate) fn new_dep(d: dep::Dep<PkgDep>) -> Self {
        Self {
            unit: DepUnit::PkgDep,
            kind: DepKind::from(&d),
            dep: Box::into_raw(Box::new(DepW::PkgDep(d))),
        }
    }

    pub(crate) fn new_string(d: dep::Dep<String>) -> Self {
        Self {
            unit: DepUnit::String,
            kind: DepKind::from(&d),
            dep: Box::into_raw(Box::new(DepW::String(d))),
        }
    }

    pub(crate) fn new_uri(d: dep::Dep<Uri>) -> Self {
        Self {
            unit: DepUnit::Uri,
            kind: DepKind::from(&d),
            dep: Box::into_raw(Box::new(DepW::Uri(d))),
        }
    }
}

impl Hash for Dep {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl Ord for Dep {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deref().cmp(other.deref())
    }
}

impl PartialOrd for Dep {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Dep {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Dep {}

impl Deref for Dep {
    type Target = DepW;

    fn deref(&self) -> &Self::Target {
        null_ptr_check!(self.dep.as_ref())
    }
}

impl fmt::Display for Dep {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

/// Opaque wrapper for flattened DepSet iterators.
#[derive(Debug)]
pub enum DepSetIntoIterFlatten {
    PkgDep(dep::DepSetIntoIterFlatten<PkgDep>),
    String(dep::DepSetIntoIterFlatten<String>),
    Uri(dep::DepSetIntoIterFlatten<Uri>),
}

impl Iterator for DepSetIntoIterFlatten {
    type Item = *mut c_void;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::PkgDep(iter) => iter
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
    PkgDep(dep::DepSetIntoIterRecursive<PkgDep>),
    String(dep::DepSetIntoIterRecursive<String>),
    Uri(dep::DepSetIntoIterRecursive<Uri>),
}

impl Iterator for DepSetIntoIterRecursive {
    type Item = Dep;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::PkgDep(iter) => iter.next().map(Dep::new_dep),
            Self::String(iter) => iter.next().map(Dep::new_string),
            Self::Uri(iter) => iter.next().map(Dep::new_uri),
        }
    }
}

/// Parse a string into a Dependencies DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_depset_dependencies(
    s: *const c_char,
    eapi: *const Eapi,
) -> *mut DepSet {
    let s = null_ptr_check!(s.as_ref());
    let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
    let eapi = unwrap_or_return!(IntoEapi::into_eapi(eapi), ptr::null_mut());
    let opt_dep = unwrap_or_return!(dep::parse::dependencies(s, eapi), ptr::null_mut());
    let dep = DepSet::new_dep(opt_dep.unwrap_or_default());
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
    let opt_dep = unwrap_or_return!(dep::parse::restrict(s), ptr::null_mut());
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
    let opt_dep = unwrap_or_return!(dep::parse::required_use(s, eapi), ptr::null_mut());
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
    let opt_dep = unwrap_or_return!(dep::parse::properties(s), ptr::null_mut());
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
    let opt_dep = unwrap_or_return!(dep::parse::src_uri(s, eapi), ptr::null_mut());
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
    let opt_dep = unwrap_or_return!(dep::parse::license(s), ptr::null_mut());
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
        DepSetW::PkgDep(d) => DepSetIntoIter::PkgDep(d.into_iter()),
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
pub unsafe extern "C" fn pkgcraft_depset_into_iter_next(i: *mut DepSetIntoIter) -> *mut Dep {
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

/// Compare two Deps returning -1, 0, or 1 if the first is less than, equal to, or greater
/// than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Dep pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_cmp(d1: *mut Dep, d2: *mut Dep) -> c_int {
    let d1 = null_ptr_check!(d1.as_ref());
    let d2 = null_ptr_check!(d2.as_ref());

    match d1.cmp(d2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the hash value for a Dep.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_hash(d: *mut Dep) -> u64 {
    let deps = null_ptr_check!(d.as_ref());
    hash(deps)
}

/// Free a Dep object.
///
/// # Safety
/// The argument must be a Dep pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_free(r: *mut Dep) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}

/// Return the formatted string for a Dep object.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_str(d: *mut Dep) -> *mut c_char {
    let deps = null_ptr_check!(d.as_ref());
    CString::new(deps.to_string()).unwrap().into_raw()
}

/// Return a flattened iterator for a Dep.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_into_iter_flatten(
    d: *mut Dep,
) -> *mut DepSetIntoIterFlatten {
    let dep = null_ptr_check!(d.as_ref());
    let iter = match dep.deref().clone() {
        DepW::PkgDep(d) => DepSetIntoIterFlatten::PkgDep(d.into_iter_flatten()),
        DepW::String(d) => DepSetIntoIterFlatten::String(d.into_iter_flatten()),
        DepW::Uri(d) => DepSetIntoIterFlatten::Uri(d.into_iter_flatten()),
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
        DepSetW::PkgDep(d) => DepSetIntoIterFlatten::PkgDep(d.into_iter_flatten()),
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

/// Return a recursive iterator for a Dep.
///
/// # Safety
/// The argument must be a non-null Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_deprestrict_into_iter_recursive(
    d: *mut Dep,
) -> *mut DepSetIntoIterRecursive {
    let dep = null_ptr_check!(d.as_ref());
    let iter = match dep.deref().clone() {
        DepW::PkgDep(d) => DepSetIntoIterRecursive::PkgDep(d.into_iter_recursive()),
        DepW::String(d) => DepSetIntoIterRecursive::String(d.into_iter_recursive()),
        DepW::Uri(d) => DepSetIntoIterRecursive::Uri(d.into_iter_recursive()),
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
        DepSetW::PkgDep(d) => DepSetIntoIterRecursive::PkgDep(d.into_iter_recursive()),
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
) -> *mut Dep {
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
