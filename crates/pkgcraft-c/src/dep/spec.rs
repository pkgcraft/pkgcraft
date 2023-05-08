use std::cmp::Ordering;
use std::ffi::{c_char, c_int, c_void};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::{fmt, ptr};

use pkgcraft::dep::{self, Dep, Flatten, Recursive, Uri};
use pkgcraft::eapi::{Eapi, IntoEapi};
use pkgcraft::types::Ordered;
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// DepSpec unit variants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum DepSpecUnit {
    Dep,
    String,
    Uri,
}

/// DepSet variants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum DepSetKind {
    Dependencies,
    License,
    Properties,
    RequiredUse,
    Restrict,
    SrcUri,
}

/// Opaque wrapper for pkgcraft::dep::DepSet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DepSetWrapper {
    Dep(dep::DepSet<Dep>),
    String(dep::DepSet<String>),
    Uri(dep::DepSet<Uri>),
}

impl fmt::Display for DepSetWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Dep(d) => write!(f, "{d}"),
            Self::String(d) => write!(f, "{d}"),
            Self::Uri(d) => write!(f, "{d}"),
        }
    }
}

/// C-compatible wrapper for pkgcraft::dep::DepSet.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DepSet {
    unit: DepSpecUnit,
    kind: DepSetKind,
    dep: *mut DepSetWrapper,
}

impl Drop for DepSet {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.dep));
        }
    }
}

impl DepSet {
    pub(crate) fn new_dep(d: dep::DepSet<Dep>) -> Self {
        Self {
            unit: DepSpecUnit::Dep,
            kind: DepSetKind::Dependencies,
            dep: Box::into_raw(Box::new(DepSetWrapper::Dep(d))),
        }
    }

    pub(crate) fn new_string(d: dep::DepSet<String>, kind: DepSetKind) -> Self {
        Self {
            unit: DepSpecUnit::String,
            kind,
            dep: Box::into_raw(Box::new(DepSetWrapper::String(d))),
        }
    }

    pub(crate) fn new_uri(d: dep::DepSet<Uri>) -> Self {
        Self {
            unit: DepSpecUnit::Uri,
            kind: DepSetKind::SrcUri,
            dep: Box::into_raw(Box::new(DepSetWrapper::Uri(d))),
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
    type Target = DepSetWrapper;

    fn deref(&self) -> &Self::Target {
        try_ref_from_ptr!(self.dep)
    }
}

impl fmt::Display for DepSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

/// Opaque wrapper for pkgcraft::dep::spec::IntoIter<T>.
#[derive(Debug)]
pub enum DepSpecIntoIter {
    Dep(dep::spec::IntoIter<Dep>),
    String(dep::spec::IntoIter<String>),
    Uri(dep::spec::IntoIter<Uri>),
}

impl Iterator for DepSpecIntoIter {
    type Item = DepSpec;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(iter) => iter.next().map(DepSpec::new_dep),
            Self::String(iter) => iter.next().map(DepSpec::new_string),
            Self::Uri(iter) => iter.next().map(DepSpec::new_uri),
        }
    }
}

/// Opaque wrapper for pkgcraft::dep::DepSpec.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DepSpecWrapper {
    Dep(dep::DepSpec<Dep>),
    String(dep::DepSpec<String>),
    Uri(dep::DepSpec<Uri>),
}

impl fmt::Display for DepSpecWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Dep(d) => write!(f, "{d}"),
            Self::String(d) => write!(f, "{d}"),
            Self::Uri(d) => write!(f, "{d}"),
        }
    }
}

/// DepSpec variants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum DepSpecKind {
    Enabled,
    Disabled,
    AllOf,
    AnyOf,
    ExactlyOneOf,
    AtMostOneOf,
    UseEnabled,
    UseDisabled,
}

impl<T: Ordered> From<&dep::DepSpec<T>> for DepSpecKind {
    fn from(d: &dep::DepSpec<T>) -> Self {
        use dep::DepSpec::*;
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

/// C-compatible wrapper for pkgcraft::dep::DepSpec.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DepSpec {
    unit: DepSpecUnit,
    kind: DepSpecKind,
    dep: *mut DepSpecWrapper,
}

impl Drop for DepSpec {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.dep));
        }
    }
}

impl DepSpec {
    pub(crate) fn new_dep(d: dep::DepSpec<Dep>) -> Self {
        Self {
            unit: DepSpecUnit::Dep,
            kind: DepSpecKind::from(&d),
            dep: Box::into_raw(Box::new(DepSpecWrapper::Dep(d))),
        }
    }

    pub(crate) fn new_string(d: dep::DepSpec<String>) -> Self {
        Self {
            unit: DepSpecUnit::String,
            kind: DepSpecKind::from(&d),
            dep: Box::into_raw(Box::new(DepSpecWrapper::String(d))),
        }
    }

    pub(crate) fn new_uri(d: dep::DepSpec<Uri>) -> Self {
        Self {
            unit: DepSpecUnit::Uri,
            kind: DepSpecKind::from(&d),
            dep: Box::into_raw(Box::new(DepSpecWrapper::Uri(d))),
        }
    }
}

impl Hash for DepSpec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl Ord for DepSpec {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deref().cmp(other.deref())
    }
}

impl PartialOrd for DepSpec {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for DepSpec {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for DepSpec {}

impl Deref for DepSpec {
    type Target = DepSpecWrapper;

    fn deref(&self) -> &Self::Target {
        try_ref_from_ptr!(self.dep)
    }
}

impl fmt::Display for DepSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

/// Opaque wrapper for pkgcraft::dep::spec::IntoIterFlatten<T>.
#[derive(Debug)]
pub enum DepSpecIntoIterFlatten {
    Dep(dep::spec::IntoIterFlatten<Dep>),
    String(dep::spec::IntoIterFlatten<String>),
    Uri(dep::spec::IntoIterFlatten<Uri>),
}

impl Iterator for DepSpecIntoIterFlatten {
    type Item = *mut c_void;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(iter) => iter
                .next()
                .map(|x| Box::into_raw(Box::new(x)) as *mut c_void),
            Self::String(iter) => iter
                .next()
                .map(|x| try_ptr_from_str!(x.as_str()) as *mut c_void),
            Self::Uri(iter) => iter
                .next()
                .map(|x| Box::into_raw(Box::new(x)) as *mut c_void),
        }
    }
}

/// Opaque wrapper for pkgcraft::dep::spec::IntoIterRecursive<T>.
#[derive(Debug)]
pub enum DepSpecIntoIterRecursive {
    Dep(dep::spec::IntoIterRecursive<Dep>),
    String(dep::spec::IntoIterRecursive<String>),
    Uri(dep::spec::IntoIterRecursive<Uri>),
}

impl Iterator for DepSpecIntoIterRecursive {
    type Item = DepSpec;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(iter) => iter.next().map(DepSpec::new_dep),
            Self::String(iter) => iter.next().map(DepSpec::new_string),
            Self::Uri(iter) => iter.next().map(DepSpec::new_uri),
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
pub unsafe extern "C" fn pkgcraft_dep_set_dependencies(
    s: *const c_char,
    eapi: *const Eapi,
) -> *mut DepSet {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let eapi = unwrap_or_panic!(IntoEapi::into_eapi(eapi));
        let opt_dep = unwrap_or_panic!(dep::parse::dependencies(s, eapi));
        let dep = DepSet::new_dep(opt_dep.unwrap_or_default());
        Box::into_raw(Box::new(dep))
    }
}

/// Parse a string into a Restrict DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_restrict(s: *const c_char) -> *mut DepSet {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let opt_dep = unwrap_or_panic!(dep::parse::restrict(s));
        let dep = DepSet::new_string(opt_dep.unwrap_or_default(), DepSetKind::Restrict);
        Box::into_raw(Box::new(dep))
    }
}

/// Parse a string into a RequiredUse DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_required_use(
    s: *const c_char,
    eapi: *const Eapi,
) -> *mut DepSet {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let eapi = unwrap_or_panic!(IntoEapi::into_eapi(eapi));
        let opt_dep = unwrap_or_panic!(dep::parse::required_use(s, eapi));
        let dep = DepSet::new_string(opt_dep.unwrap_or_default(), DepSetKind::RequiredUse);
        Box::into_raw(Box::new(dep))
    }
}

/// Parse a string into a Properties DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_properties(s: *const c_char) -> *mut DepSet {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let opt_dep = unwrap_or_panic!(dep::parse::properties(s));
        let dep = DepSet::new_string(opt_dep.unwrap_or_default(), DepSetKind::Properties);
        Box::into_raw(Box::new(dep))
    }
}

/// Parse a string into a SrcUri DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_src_uri(
    s: *const c_char,
    eapi: *const Eapi,
) -> *mut DepSet {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let eapi = unwrap_or_panic!(IntoEapi::into_eapi(eapi));
        let opt_dep = unwrap_or_panic!(dep::parse::src_uri(s, eapi));
        let dep = DepSet::new_uri(opt_dep.unwrap_or_default());
        Box::into_raw(Box::new(dep))
    }
}

/// Parse a string into a License DepSet.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_license(s: *const c_char) -> *mut DepSet {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let opt_dep = unwrap_or_panic!(dep::parse::license(s));
        let dep = DepSet::new_string(opt_dep.unwrap_or_default(), DepSetKind::License);
        Box::into_raw(Box::new(dep))
    }
}

/// Return the formatted string for a DepSet object.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_str(d: *mut DepSet) -> *mut c_char {
    let deps = try_ref_from_ptr!(d);
    try_ptr_from_str!(deps.to_string())
}

/// Determine if two DepSets are equal.
///
/// # Safety
/// The arguments must be non-null DepSet pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_eq(d1: *mut DepSet, d2: *mut DepSet) -> bool {
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);
    d1.eq(d2)
}

/// Return the hash value for a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_hash(d: *mut DepSet) -> u64 {
    let deps = try_ref_from_ptr!(d);
    hash(deps)
}

/// Return an iterator for a depset.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter(d: *mut DepSet) -> *mut DepSpecIntoIter {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DepSetWrapper::Dep(d) => DepSpecIntoIter::Dep(d.into_iter()),
        DepSetWrapper::String(d) => DepSpecIntoIter::String(d.into_iter()),
        DepSetWrapper::Uri(d) => DepSpecIntoIter::Uri(d.into_iter()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a depset iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIter pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_next(i: *mut DepSpecIntoIter) -> *mut DepSpec {
    let iter = try_mut_from_ptr!(i);
    iter.next()
        .map(|x| Box::into_raw(Box::new(x)))
        .unwrap_or(ptr::null_mut())
}

/// Free a depset iterator.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIter pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_free(i: *mut DepSpecIntoIter) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Compare two DepSpecs returning -1, 0, or 1 if the first is less than, equal to, or greater
/// than the second, respectively.
///
/// # Safety
/// The arguments must be non-null DepSpec pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_cmp(d1: *mut DepSpec, d2: *mut DepSpec) -> c_int {
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);

    match d1.cmp(d2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the hash value for a DepSpec.
///
/// # Safety
/// The argument must be a non-null DepSpec pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_hash(d: *mut DepSpec) -> u64 {
    let deps = try_ref_from_ptr!(d);
    hash(deps)
}

/// Free a DepSpec object.
///
/// # Safety
/// The argument must be a DepSpec pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_free(r: *mut DepSpec) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}

/// Return the formatted string for a DepSpec object.
///
/// # Safety
/// The argument must be a non-null DepSpec pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_str(d: *mut DepSpec) -> *mut c_char {
    let deps = try_ref_from_ptr!(d);
    try_ptr_from_str!(deps.to_string())
}

/// Return a flattened iterator for a DepSpec.
///
/// # Safety
/// The argument must be a non-null DepSpec pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_into_iter_flatten(
    d: *mut DepSpec,
) -> *mut DepSpecIntoIterFlatten {
    let dep = try_ref_from_ptr!(d);
    let iter = match dep.deref().clone() {
        DepSpecWrapper::Dep(d) => DepSpecIntoIterFlatten::Dep(d.into_iter_flatten()),
        DepSpecWrapper::String(d) => DepSpecIntoIterFlatten::String(d.into_iter_flatten()),
        DepSpecWrapper::Uri(d) => DepSpecIntoIterFlatten::Uri(d.into_iter_flatten()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return a flattened iterator for a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_flatten(
    d: *mut DepSet,
) -> *mut DepSpecIntoIterFlatten {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DepSetWrapper::Dep(d) => DepSpecIntoIterFlatten::Dep(d.into_iter_flatten()),
        DepSetWrapper::String(d) => DepSpecIntoIterFlatten::String(d.into_iter_flatten()),
        DepSetWrapper::Uri(d) => DepSpecIntoIterFlatten::Uri(d.into_iter_flatten()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a flattened depset iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIterFlatten pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_flatten_next(
    i: *mut DepSpecIntoIterFlatten,
) -> *mut c_void {
    let iter = try_mut_from_ptr!(i);
    iter.next().unwrap_or(ptr::null_mut())
}

/// Free a flattened depset iterator.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIterFlatten pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_flatten_free(i: *mut DepSpecIntoIterFlatten) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Return a recursive iterator for a DepSpec.
///
/// # Safety
/// The argument must be a non-null DepSpec pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_into_iter_recursive(
    d: *mut DepSpec,
) -> *mut DepSpecIntoIterRecursive {
    let dep = try_ref_from_ptr!(d);
    let iter = match dep.deref().clone() {
        DepSpecWrapper::Dep(d) => DepSpecIntoIterRecursive::Dep(d.into_iter_recursive()),
        DepSpecWrapper::String(d) => DepSpecIntoIterRecursive::String(d.into_iter_recursive()),
        DepSpecWrapper::Uri(d) => DepSpecIntoIterRecursive::Uri(d.into_iter_recursive()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return a recursive iterator for a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_recursive(
    d: *mut DepSet,
) -> *mut DepSpecIntoIterRecursive {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DepSetWrapper::Dep(d) => DepSpecIntoIterRecursive::Dep(d.into_iter_recursive()),
        DepSetWrapper::String(d) => DepSpecIntoIterRecursive::String(d.into_iter_recursive()),
        DepSetWrapper::Uri(d) => DepSpecIntoIterRecursive::Uri(d.into_iter_recursive()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a recursive depset iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIterRecursive pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_recursive_next(
    i: *mut DepSpecIntoIterRecursive,
) -> *mut DepSpec {
    let iter = try_mut_from_ptr!(i);
    iter.next()
        .map(|x| Box::into_raw(Box::new(x)))
        .unwrap_or(ptr::null_mut())
}

/// Free a recursive depset iterator.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIterFlatten pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_recursive_free(
    i: *mut DepSpecIntoIterRecursive,
) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Free a DepSet.
///
/// # Safety
/// The argument must be a DepSet pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_free(d: *mut DepSet) {
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
    let uri = try_ref_from_ptr!(u);
    try_ptr_from_str!(uri.uri())
}

/// Get the filename for a Uri.
///
/// # Safety
/// The argument must be a Uri pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri_filename(u: *mut Uri) -> *mut c_char {
    let uri = try_ref_from_ptr!(u);
    try_ptr_from_str!(uri.filename())
}

/// Return the formatted string for a Uri object.
///
/// # Safety
/// The argument must be a Uri pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_uri_str(u: *mut Uri) -> *mut c_char {
    let uri = try_ref_from_ptr!(u);
    try_ptr_from_str!(uri.to_string())
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
