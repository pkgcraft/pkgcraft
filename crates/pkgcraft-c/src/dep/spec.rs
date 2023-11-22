use std::cmp::Ordering;
use std::ffi::{c_char, c_int, c_void};
use std::hash::{Hash, Hasher};
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Sub, SubAssign,
};
use std::{fmt, ptr, slice};

use pkgcraft::dep::{
    self, Conditionals, Dep, Evaluate, EvaluateForce, Flatten, IntoOwned, Recursive, Uri, UseFlag,
};
use pkgcraft::eapi::Eapi;
use pkgcraft::traits::Contains;
use pkgcraft::types::Ordered;
use pkgcraft::utils::hash;

use crate::eapi::eapi_or_default;
use crate::error::Error;
use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::types::SetOp;
use crate::utils::boxed;

/// DepSet variants.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum DepSetKind {
    #[default]
    Dependencies,
    SrcUri,
    License,
    Properties,
    RequiredUse,
    Restrict,
}

/// Opaque wrapper for pkgcraft::dep::DepSet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DepSetWrapper {
    Dep(dep::DepSet<String, Dep>),
    String(dep::DepSet<String, String>),
    Uri(dep::DepSet<String, Uri>),
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
#[derive(Debug)]
#[repr(C)]
pub struct DepSet {
    set: DepSetKind,
    dep: *mut DepSetWrapper,
}

impl Clone for DepSet {
    fn clone(&self) -> Self {
        let dep = try_ref_from_ptr!(self.dep);
        Self {
            set: self.set,
            dep: Box::into_raw(Box::new(dep.clone())),
        }
    }
}

impl Drop for DepSet {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.dep));
        }
    }
}

impl DepSet {
    pub(crate) fn new_dep(d: dep::DepSet<String, Dep>) -> Self {
        Self {
            set: DepSetKind::Dependencies,
            dep: Box::into_raw(Box::new(DepSetWrapper::Dep(d))),
        }
    }

    pub(crate) fn new_string(d: dep::DepSet<String, String>, set: DepSetKind) -> Self {
        Self {
            set,
            dep: Box::into_raw(Box::new(DepSetWrapper::String(d))),
        }
    }

    pub(crate) fn new_uri(d: dep::DepSet<String, Uri>) -> Self {
        Self {
            set: DepSetKind::SrcUri,
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

impl DerefMut for DepSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        try_mut_from_ptr!(self.dep)
    }
}

impl fmt::Display for DepSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

impl BitAnd<&DepSet> for &DepSet {
    type Output = DepSet;

    fn bitand(self, other: &DepSet) -> Self::Output {
        let mut dep = self.clone();
        dep &= other;
        dep
    }
}

impl BitAndAssign<&DepSet> for DepSet {
    fn bitand_assign(&mut self, other: &DepSet) {
        use DepSetWrapper::*;
        match (self.deref_mut(), other.deref()) {
            (Dep(d1), Dep(d2)) => *d1 &= d2,
            (String(d1), String(d2)) => *d1 &= d2,
            (Uri(d1), Uri(d2)) => *d1 &= d2,
            _ => {
                set_error_and_panic!(Error::new(format!(
                    "DepSet kind {:?} doesn't match: {:?}",
                    self.set, other.set
                )));
            }
        }
    }
}

impl BitOr<&DepSet> for &DepSet {
    type Output = DepSet;

    fn bitor(self, other: &DepSet) -> Self::Output {
        let mut dep = self.clone();
        dep |= other;
        dep
    }
}

impl BitOrAssign<&DepSet> for DepSet {
    fn bitor_assign(&mut self, other: &DepSet) {
        use DepSetWrapper::*;
        match (self.deref_mut(), other.deref()) {
            (Dep(d1), Dep(d2)) => *d1 |= d2,
            (String(d1), String(d2)) => *d1 |= d2,
            (Uri(d1), Uri(d2)) => *d1 |= d2,
            _ => {
                set_error_and_panic!(Error::new(format!(
                    "DepSet kind {:?} doesn't match: {:?}",
                    self.set, other.set
                )));
            }
        }
    }
}

impl BitXor<&DepSet> for &DepSet {
    type Output = DepSet;

    fn bitxor(self, other: &DepSet) -> Self::Output {
        let mut dep = self.clone();
        dep ^= other;
        dep
    }
}

impl BitXorAssign<&DepSet> for DepSet {
    fn bitxor_assign(&mut self, other: &DepSet) {
        use DepSetWrapper::*;
        match (self.deref_mut(), other.deref()) {
            (Dep(d1), Dep(d2)) => *d1 ^= d2,
            (String(d1), String(d2)) => *d1 ^= d2,
            (Uri(d1), Uri(d2)) => *d1 ^= d2,
            _ => {
                set_error_and_panic!(Error::new(format!(
                    "DepSet kind {:?} doesn't match: {:?}",
                    self.set, other.set
                )));
            }
        }
    }
}

impl Sub<&DepSet> for &DepSet {
    type Output = DepSet;

    fn sub(self, other: &DepSet) -> Self::Output {
        let mut dep = self.clone();
        dep -= other;
        dep
    }
}

impl SubAssign<&DepSet> for DepSet {
    fn sub_assign(&mut self, other: &DepSet) {
        use DepSetWrapper::*;
        match (self.deref_mut(), other.deref()) {
            (Dep(d1), Dep(d2)) => *d1 -= d2,
            (String(d1), String(d2)) => *d1 -= d2,
            (Uri(d1), Uri(d2)) => *d1 -= d2,
            _ => {
                set_error_and_panic!(Error::new(format!(
                    "DepSet kind {:?} doesn't match: {:?}",
                    self.set, other.set
                )));
            }
        }
    }
}

/// Opaque wrapper for pkgcraft::dep::spec::IntoIter<String, T>.
#[derive(Debug)]
pub enum DepSpecIntoIter {
    Dep(DepSetKind, dep::spec::IntoIter<String, Dep>),
    String(DepSetKind, dep::spec::IntoIter<String, String>),
    Uri(DepSetKind, dep::spec::IntoIter<String, Uri>),
}

impl Iterator for DepSpecIntoIter {
    type Item = DepSpec;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(_, iter) => iter.next().map(DepSpec::new_dep),
            Self::String(set, iter) => iter.next().map(|d| DepSpec::new_string(d, *set)),
            Self::Uri(_, iter) => iter.next().map(DepSpec::new_uri),
        }
    }
}

impl DoubleEndedIterator for DepSpecIntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(_, iter) => iter.next_back().map(DepSpec::new_dep),
            Self::String(set, iter) => iter.next_back().map(|d| DepSpec::new_string(d, *set)),
            Self::Uri(_, iter) => iter.next_back().map(DepSpec::new_uri),
        }
    }
}

/// Opaque wrapper for pkgcraft::dep::DepSpec.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DepSpecWrapper {
    Dep(dep::DepSpec<String, Dep>),
    String(dep::DepSpec<String, String>),
    Uri(dep::DepSpec<String, Uri>),
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
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl<S: UseFlag, T: Ordered> From<&dep::DepSpec<S, T>> for DepSpecKind {
    fn from(d: &dep::DepSpec<S, T>) -> Self {
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
    set: DepSetKind,
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
    pub(crate) fn new_dep(d: dep::DepSpec<String, Dep>) -> Self {
        Self {
            set: DepSetKind::Dependencies,
            kind: DepSpecKind::from(&d),
            dep: Box::into_raw(Box::new(DepSpecWrapper::Dep(d))),
        }
    }

    pub(crate) fn new_string(d: dep::DepSpec<String, String>, set: DepSetKind) -> Self {
        Self {
            set,
            kind: DepSpecKind::from(&d),
            dep: Box::into_raw(Box::new(DepSpecWrapper::String(d))),
        }
    }

    pub(crate) fn new_uri(d: dep::DepSpec<String, Uri>) -> Self {
        Self {
            set: DepSetKind::SrcUri,
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
        self.deref().eq(other.deref())
    }
}

impl Eq for DepSpec {}

impl Deref for DepSpec {
    type Target = DepSpecWrapper;

    fn deref(&self) -> &Self::Target {
        try_ref_from_ptr!(self.dep)
    }
}

impl DerefMut for DepSpec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        try_mut_from_ptr!(self.dep)
    }
}

impl fmt::Display for DepSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

/// Opaque wrapper for pkgcraft::dep::spec::IntoIterFlatten<String, T>.
#[derive(Debug)]
pub enum DepSpecIntoIterFlatten {
    Dep(dep::spec::IntoIterFlatten<String, Dep>),
    String(dep::spec::IntoIterFlatten<String, String>),
    Uri(dep::spec::IntoIterFlatten<String, Uri>),
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

/// Opaque wrapper for pkgcraft::dep::spec::IntoIterRecursive<String, T>.
#[derive(Debug)]
pub enum DepSpecIntoIterRecursive {
    Dep(DepSetKind, dep::spec::IntoIterRecursive<String, Dep>),
    String(DepSetKind, dep::spec::IntoIterRecursive<String, String>),
    Uri(DepSetKind, dep::spec::IntoIterRecursive<String, Uri>),
}

impl Iterator for DepSpecIntoIterRecursive {
    type Item = DepSpec;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(_, iter) => iter.next().map(DepSpec::new_dep),
            Self::String(set, iter) => iter.next().map(|d| DepSpec::new_string(d, *set)),
            Self::Uri(_, iter) => iter.next().map(DepSpec::new_uri),
        }
    }
}

/// Opaque wrapper for pkgcraft::dep::spec::IntoIterConditionals<String, T>.
#[derive(Debug)]
pub enum DepSpecIntoIterConditionals {
    Dep(dep::spec::IntoIterConditionals<String, Dep>),
    String(dep::spec::IntoIterConditionals<String, String>),
    Uri(dep::spec::IntoIterConditionals<String, Uri>),
}

impl Iterator for DepSpecIntoIterConditionals {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(iter) => iter.next(),
            Self::String(iter) => iter.next(),
            Self::Uri(iter) => iter.next(),
        }
    }
}

/// Create a DepSet from an array of DepSpec objects.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be an array of similarly-typed DepSpec objects.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_from_iter(
    deps: *mut *mut DepSpec,
    len: usize,
    kind: DepSetKind,
) -> *mut DepSet {
    ffi_catch_panic! {
        let deps = unsafe { slice::from_raw_parts(deps, len) };
        let deps = deps.iter().map(|p| try_ref_from_ptr!(p));
        let (mut deps_dep, mut deps_string, mut deps_uri) = (vec![], vec![], vec![]);

        for d in deps {
            if d.set != kind {
                set_error_and_panic!(
                    Error::new(format!("DepSpec kind {:?} doesn't match: {kind:?}", d.set))
                );
            }

            match d.deref() {
                DepSpecWrapper::Dep(d) => deps_dep.push(d.clone()),
                DepSpecWrapper::String(d) => deps_string.push(d.clone()),
                DepSpecWrapper::Uri(d) => deps_uri.push(d.clone()),
            }
        }

        use DepSetKind::*;
        let dep = match kind {
            Dependencies => DepSet::new_dep(deps_dep.into_iter().collect()),
            SrcUri => DepSet::new_uri(deps_uri.into_iter().collect()),
            _ => DepSet::new_string(deps_string.into_iter().collect(), kind),
        };

        Box::into_raw(Box::new(dep))
    }
}

/// Parse a string into a specified DepSet type.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_parse(
    s: *const c_char,
    eapi: *const Eapi,
    kind: DepSetKind,
) -> *mut DepSet {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let eapi = eapi_or_default!(eapi);

        use DepSetKind::*;
        let depset = match kind {
            Dependencies => {
                let opt_dep = unwrap_or_panic!(dep::parse::dependencies_dep_set(s, eapi));
                DepSet::new_dep(opt_dep.unwrap_or_default())
            },
            SrcUri => {
                let opt_dep = unwrap_or_panic!(dep::parse::src_uri_dep_set(s, eapi));
                DepSet::new_uri(opt_dep.unwrap_or_default())
            },
            License => {
                let opt_dep = unwrap_or_panic!(dep::parse::license_dep_set(s));
                DepSet::new_string(opt_dep.unwrap_or_default(), kind)
            },
            Properties => {
                let opt_dep = unwrap_or_panic!(dep::parse::properties_dep_set(s));
                DepSet::new_string(opt_dep.unwrap_or_default(), kind)
            },
            RequiredUse => {
                let opt_dep = unwrap_or_panic!(dep::parse::required_use_dep_set(s, eapi));
                DepSet::new_string(opt_dep.unwrap_or_default(), kind)
            },
            Restrict => {
                let opt_dep = unwrap_or_panic!(dep::parse::restrict_dep_set(s));
                DepSet::new_string(opt_dep.unwrap_or_default(), kind)
            },
        };

        Box::into_raw(Box::new(depset))
    }
}

/// Parse a string into a specified DepSpec type.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_parse(
    s: *const c_char,
    eapi: *const Eapi,
    kind: DepSetKind,
) -> *mut DepSpec {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let eapi = eapi_or_default!(eapi);

        use DepSetKind::*;
        let dep = match kind {
            Dependencies => {
                let dep = unwrap_or_panic!(dep::parse::dependencies_dep_spec(s, eapi));
                DepSpec::new_dep(dep)
            },
            SrcUri => {
                let dep = unwrap_or_panic!(dep::parse::src_uri_dep_spec(s, eapi));
                DepSpec::new_uri(dep)
            },
            License => {
                let dep = unwrap_or_panic!(dep::parse::license_dep_spec(s));
                DepSpec::new_string(dep, kind)
            },
            Properties => {
                let dep = unwrap_or_panic!(dep::parse::properties_dep_spec(s));
                DepSpec::new_string(dep, kind)
            },
            RequiredUse => {
                let dep = unwrap_or_panic!(dep::parse::required_use_dep_spec(s, eapi));
                DepSpec::new_string(dep, kind)
            },
            Restrict => {
                let dep = unwrap_or_panic!(dep::parse::restrict_dep_spec(s));
                DepSpec::new_string(dep, kind)
            },
        };

        Box::into_raw(Box::new(dep))
    }
}

/// Create a DepSpec from a Dep.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be valid Dep pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_from_dep(d: *mut Dep) -> *mut DepSpec {
    ffi_catch_panic! {
        let dep = try_ref_from_ptr!(d);
        let spec = DepSpec::new_dep(dep.clone().into());
        Box::into_raw(Box::new(spec))
    }
}

/// Evaluate a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_evaluate(
    d: *mut DepSet,
    options: *mut *mut c_char,
    len: usize,
) -> *mut DepSet {
    let dep = try_ref_from_ptr!(d);
    let options = unsafe { slice::from_raw_parts(options, len) };
    let options = options.iter().map(|p| try_str_from_ptr!(p)).collect();

    use DepSetWrapper::*;
    let evaluated = match dep.deref() {
        Dep(d) => Dep(d.evaluate(&options).into_owned()),
        String(d) => String(d.evaluate(&options).into_owned()),
        Uri(d) => Uri(d.evaluate(&options).into_owned()),
    };

    let dep = DepSet {
        set: dep.set,
        dep: Box::into_raw(Box::new(evaluated)),
    };

    Box::into_raw(Box::new(dep))
}

/// Forcibly evaluate a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_evaluate_force(
    d: *mut DepSet,
    force: bool,
) -> *mut DepSet {
    let dep = try_ref_from_ptr!(d);

    use DepSetWrapper::*;
    let evaluated = match dep.deref() {
        Dep(d) => Dep(d.evaluate_force(force).into_owned()),
        String(d) => String(d.evaluate_force(force).into_owned()),
        Uri(d) => Uri(d.evaluate_force(force).into_owned()),
    };

    let dep = DepSet {
        set: dep.set,
        dep: Box::into_raw(Box::new(evaluated)),
    };

    Box::into_raw(Box::new(dep))
}

/// Returns true if two DepSets have no elements in common.
///
/// # Safety
/// The arguments must be a non-null DepSet pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_is_disjoint(d1: *mut DepSet, d2: *mut DepSet) -> bool {
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);

    use DepSetWrapper::*;
    match (d1.deref(), d2.deref()) {
        (Dep(d1), Dep(d2)) => d1.is_disjoint(d2),
        (String(d1), String(d2)) => d1.is_disjoint(d2),
        (Uri(d1), Uri(d2)) => d1.is_disjoint(d2),
        _ => true,
    }
}

/// Returns true if a DepSet is empty.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_is_empty(d: *mut DepSet) -> bool {
    let deps = try_ref_from_ptr!(d);

    match deps.deref() {
        DepSetWrapper::Dep(d) => d.is_empty(),
        DepSetWrapper::String(d) => d.is_empty(),
        DepSetWrapper::Uri(d) => d.is_empty(),
    }
}

/// Returns true if all the elements of the first DepSet are contained in the second.
///
/// # Safety
/// The arguments must be a non-null DepSet pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_is_subset(d1: *mut DepSet, d2: *mut DepSet) -> bool {
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);

    use DepSetWrapper::*;
    match (d1.deref(), d2.deref()) {
        (Dep(d1), Dep(d2)) => d1.is_subset(d2),
        (String(d1), String(d2)) => d1.is_subset(d2),
        (Uri(d1), Uri(d2)) => d1.is_subset(d2),
        _ => false,
    }
}

/// Returns the DepSpec element for a given index.
///
/// Returns NULL on index nonexistence.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_get_index(d: *mut DepSet, index: usize) -> *mut DepSpec {
    ffi_catch_panic! {
        let set = try_ref_from_ptr!(d);
        let err = || Error::new(format!("failed getting DepSet index: {index}"));

        use DepSetWrapper::*;
        let dep = match set.deref() {
            Dep(deps) => {
                deps.get_index(index)
                    .ok_or_else(err)
                    .map(|d| DepSpec::new_dep(d.clone()))
            }
            String(deps) => {
                deps.get_index(index)
                    .ok_or_else(err)
                    .map(|d| DepSpec::new_string(d.clone(), set.set))
            }
            Uri(deps) => {
                deps.get_index(index)
                    .ok_or_else(err)
                    .map(|d| DepSpec::new_uri(d.clone()))
            }
        };

        Box::into_raw(Box::new(unwrap_or_panic!(dep)))
    }
}

/// Insert a DepSpec into a DepSet.
///
/// Returns false if an equivalent value already exists, otherwise true.
///
/// # Safety
/// The arguments must be non-null DepSet and DepSpec pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_insert(d: *mut DepSet, value: *mut DepSpec) -> bool {
    let set = try_mut_from_ptr!(d);
    let spec = try_ref_from_ptr!(value);

    match (set.deref_mut(), spec.deref().clone()) {
        (DepSetWrapper::Dep(deps), DepSpecWrapper::Dep(dep)) => deps.insert(dep),
        (DepSetWrapper::String(deps), DepSpecWrapper::String(dep)) => deps.insert(dep),
        (DepSetWrapper::Uri(deps), DepSpecWrapper::Uri(dep)) => deps.insert(dep),
        _ => panic!("invalid DepSet and DepSpec type combination"),
    }
}

/// Remove the last value from a DepSet.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_pop(d: *mut DepSet) -> *mut DepSpec {
    let set = try_mut_from_ptr!(d);

    use DepSetWrapper::*;
    let dep = match set.deref_mut() {
        Dep(deps) => deps.pop().map(DepSpec::new_dep),
        String(deps) => deps.pop().map(|d| DepSpec::new_string(d, set.set)),
        Uri(deps) => deps.pop().map(DepSpec::new_uri),
    };

    dep.map(boxed).unwrap_or(ptr::null_mut())
}

/// Replace a DepSpec for a given index in a DepSet, returning the replaced value.
///
/// Returns NULL on index nonexistence or if the DepSet already contains the given DepSpec.
///
/// # Safety
/// The arguments must be non-null DepSet and DepSpec pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_replace_index(
    d: *mut DepSet,
    index: usize,
    value: *mut DepSpec,
) -> *mut DepSpec {
    let set = try_mut_from_ptr!(d);
    let spec = try_ref_from_ptr!(value);

    let dep = match (set.deref_mut(), spec.deref()) {
        (DepSetWrapper::Dep(deps), DepSpecWrapper::Dep(dep)) => deps
            .shift_replace_index(index, dep.clone())
            .map(DepSpec::new_dep),
        (DepSetWrapper::String(deps), DepSpecWrapper::String(dep)) => deps
            .shift_replace_index(index, dep.clone())
            .map(|d| DepSpec::new_string(d, set.set)),
        (DepSetWrapper::Uri(deps), DepSpecWrapper::Uri(dep)) => deps
            .shift_replace_index(index, dep.clone())
            .map(DepSpec::new_uri),
        _ => panic!("invalid DepSet and DepSpec type combination"),
    };

    dep.map(boxed).unwrap_or(ptr::null_mut())
}

/// Replace a DepSpec with another DepSpec in a DepSet, returning the replaced value.
///
/// Returns NULL on nonexistence or if the DepSet already contains the given DepSpec.
///
/// # Safety
/// The arguments must be non-null DepSet and DepSpec pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_replace(
    d: *mut DepSet,
    key: *const DepSpec,
    value: *mut DepSpec,
) -> *mut DepSpec {
    let set = try_mut_from_ptr!(d);
    let key = try_ref_from_ptr!(key);
    let value = try_ref_from_ptr!(value);

    let dep = match (set.deref_mut(), key.deref(), value.deref()) {
        (DepSetWrapper::Dep(deps), DepSpecWrapper::Dep(k), DepSpecWrapper::Dep(v)) => {
            deps.shift_replace(k, v.clone()).map(DepSpec::new_dep)
        }
        (DepSetWrapper::String(deps), DepSpecWrapper::String(k), DepSpecWrapper::String(v)) => deps
            .shift_replace(k, v.clone())
            .map(|d| DepSpec::new_string(d, set.set)),
        (DepSetWrapper::Uri(deps), DepSpecWrapper::Uri(k), DepSpecWrapper::Uri(v)) => {
            deps.shift_replace(k, v.clone()).map(DepSpec::new_uri)
        }
        _ => panic!("invalid DepSet and DepSpec type combination"),
    };

    dep.map(boxed).unwrap_or(ptr::null_mut())
}

/// Perform a set operation on two DepSets, assigning to the first.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be non-null DepSet pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_assign_op_set(
    op: SetOp,
    d1: *mut DepSet,
    d2: *mut DepSet,
) -> *mut DepSet {
    ffi_catch_panic! {
        use SetOp::*;
        let dep1 = try_mut_from_ptr!(d1);
        let dep2 = try_ref_from_ptr!(d2);
        match op {
            And => *dep1 &= dep2,
            Or => *dep1 |= dep2,
            Xor => *dep1 ^= dep2,
            Sub => *dep1 -= dep2,
        }
        d1
    }
}

/// Perform a set operation on two DepSets, creating a new set.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be non-null DepSet pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_op_set(
    op: SetOp,
    d1: *mut DepSet,
    d2: *mut DepSet,
) -> *mut DepSet {
    ffi_catch_panic! {
        use SetOp::*;
        let d1 = try_ref_from_ptr!(d1);
        let d2 = try_ref_from_ptr!(d2);
        let set = match op {
            And => d1 & d2,
            Or => d1 | d2,
            Xor => d1 ^ d2,
            Sub => d1 - d2,
        };
        Box::into_raw(Box::new(set))
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

/// Determine if a DepSet contains a given DepSpec.
///
/// # Safety
/// The arguments must be non-null DepSet and DepSpec pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_contains(s: *mut DepSet, d: *mut DepSpec) -> bool {
    let s = try_ref_from_ptr!(s);
    let d = try_ref_from_ptr!(d);

    match (s.deref(), d.deref()) {
        (DepSetWrapper::Dep(s), DepSpecWrapper::Dep(d)) => s.contains(d),
        (DepSetWrapper::String(s), DepSpecWrapper::String(d)) => s.contains(d),
        (DepSetWrapper::Uri(s), DepSpecWrapper::Uri(d)) => s.contains(d),
        _ => false,
    }
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

/// Return a DepSet's length.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_len(d: *mut DepSet) -> usize {
    let deps = try_ref_from_ptr!(d);
    use DepSetWrapper::*;
    match deps.deref() {
        Dep(d) => d.len(),
        String(d) => d.len(),
        Uri(d) => d.len(),
    }
}

/// Return an iterator for a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter(d: *mut DepSet) -> *mut DepSpecIntoIter {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DepSetWrapper::Dep(d) => DepSpecIntoIter::Dep(deps.set, d.into_iter()),
        DepSetWrapper::String(d) => DepSpecIntoIter::String(deps.set, d.into_iter()),
        DepSetWrapper::Uri(d) => DepSpecIntoIter::Uri(deps.set, d.into_iter()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a DepSet iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIter pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_next(i: *mut DepSpecIntoIter) -> *mut DepSpec {
    let iter = try_mut_from_ptr!(i);
    iter.next().map(boxed).unwrap_or(ptr::null_mut())
}

/// Return the next object from the end of a DepSet iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIter pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_next_back(
    i: *mut DepSpecIntoIter,
) -> *mut DepSpec {
    let iter = try_mut_from_ptr!(i);
    iter.next_back().map(boxed).unwrap_or(ptr::null_mut())
}

/// Free a DepSet iterator.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIter pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_free(i: *mut DepSpecIntoIter) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Evaluate a DepSpec.
///
/// # Safety
/// The argument must be a non-null DepSpec pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_evaluate(
    d: *mut DepSpec,
    options: *mut *mut c_char,
    len: usize,
    deps_len: *mut usize,
) -> *mut *mut DepSpec {
    let dep = try_ref_from_ptr!(d);
    let options = unsafe { slice::from_raw_parts(options, len) };
    let options = options.iter().map(|p| try_str_from_ptr!(p)).collect();

    use DepSpecWrapper::*;
    match dep.deref() {
        Dep(d) => {
            iter_to_array!(d.evaluate(&options).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(DepSpec::new_dep(d.into_owned())))
            })
        }
        String(d) => {
            iter_to_array!(d.evaluate(&options).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(DepSpec::new_string(d.into_owned(), dep.set)))
            })
        }
        Uri(d) => {
            iter_to_array!(d.evaluate(&options).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(DepSpec::new_uri(d.into_owned())))
            })
        }
    }
}

/// Forcibly evaluate a DepSpec.
///
/// # Safety
/// The argument must be a non-null DepSpec pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_evaluate_force(
    d: *mut DepSpec,
    force: bool,
    deps_len: *mut usize,
) -> *mut *mut DepSpec {
    let dep = try_ref_from_ptr!(d);

    use DepSpecWrapper::*;
    match dep.deref() {
        Dep(d) => {
            iter_to_array!(d.evaluate_force(force).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(DepSpec::new_dep(d.into_owned())))
            })
        }
        String(d) => {
            iter_to_array!(d.evaluate_force(force).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(DepSpec::new_string(d.into_owned(), dep.set)))
            })
        }
        Uri(d) => {
            iter_to_array!(d.evaluate_force(force).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(DepSpec::new_uri(d.into_owned())))
            })
        }
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

/// Determine if a DepSpec contains a given DepSpec.
///
/// # Safety
/// The arguments must be non-null DepSpec pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_contains(d1: *mut DepSpec, d2: *mut DepSpec) -> bool {
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);

    use DepSpecWrapper::*;
    match (d1.deref(), d2.deref()) {
        (Dep(d1), Dep(d2)) => d1.contains(d2),
        (String(d1), String(d2)) => d1.contains(d2),
        (Uri(d1), Uri(d2)) => d1.contains(d2),
        _ => false,
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

/// Return a DepSpec's length.
///
/// # Safety
/// The argument must be a non-null DepSpec pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_len(d: *mut DepSpec) -> usize {
    let deps = try_ref_from_ptr!(d);
    use DepSpecWrapper::*;
    match deps.deref() {
        Dep(d) => d.len(),
        String(d) => d.len(),
        Uri(d) => d.len(),
    }
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

/// Return an iterator for a DepSpec.
///
/// # Safety
/// The argument must be a non-null DepSpec pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_into_iter(d: *mut DepSpec) -> *mut DepSpecIntoIter {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DepSpecWrapper::Dep(d) => DepSpecIntoIter::Dep(deps.set, d.into_iter()),
        DepSpecWrapper::String(d) => DepSpecIntoIter::String(deps.set, d.into_iter()),
        DepSpecWrapper::Uri(d) => DepSpecIntoIter::Uri(deps.set, d.into_iter()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return a flatten iterator for a DepSpec.
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

/// Return a flatten iterator for a DepSet.
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

/// Return the next object from a flatten iterator.
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

/// Free a flatten iterator.
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
        DepSpecWrapper::Dep(d) => DepSpecIntoIterRecursive::Dep(dep.set, d.into_iter_recursive()),
        DepSpecWrapper::String(d) => {
            DepSpecIntoIterRecursive::String(dep.set, d.into_iter_recursive())
        }
        DepSpecWrapper::Uri(d) => DepSpecIntoIterRecursive::Uri(dep.set, d.into_iter_recursive()),
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
        DepSetWrapper::Dep(d) => DepSpecIntoIterRecursive::Dep(deps.set, d.into_iter_recursive()),
        DepSetWrapper::String(d) => {
            DepSpecIntoIterRecursive::String(deps.set, d.into_iter_recursive())
        }
        DepSetWrapper::Uri(d) => DepSpecIntoIterRecursive::Uri(deps.set, d.into_iter_recursive()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a recursive iterator.
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
    iter.next().map(boxed).unwrap_or(ptr::null_mut())
}

/// Free a recursive iterator.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIterRecursive pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_recursive_free(
    i: *mut DepSpecIntoIterRecursive,
) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Return a conditionals iterator for a DepSpec.
///
/// # Safety
/// The argument must be a non-null DepSpec pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_spec_into_iter_conditionals(
    d: *mut DepSpec,
) -> *mut DepSpecIntoIterConditionals {
    let dep = try_ref_from_ptr!(d);
    let iter = match dep.deref().clone() {
        DepSpecWrapper::Dep(d) => DepSpecIntoIterConditionals::Dep(d.into_iter_conditionals()),
        DepSpecWrapper::String(d) => {
            DepSpecIntoIterConditionals::String(d.into_iter_conditionals())
        }
        DepSpecWrapper::Uri(d) => DepSpecIntoIterConditionals::Uri(d.into_iter_conditionals()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return a conditionals iterator for a DepSet.
///
/// # Safety
/// The argument must be a non-null DepSet pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_conditionals(
    d: *mut DepSet,
) -> *mut DepSpecIntoIterConditionals {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DepSetWrapper::Dep(d) => DepSpecIntoIterConditionals::Dep(d.into_iter_conditionals()),
        DepSetWrapper::String(d) => DepSpecIntoIterConditionals::String(d.into_iter_conditionals()),
        DepSetWrapper::Uri(d) => DepSpecIntoIterConditionals::Uri(d.into_iter_conditionals()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a conditionals iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIterConditionals pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_conditionals_next(
    i: *mut DepSpecIntoIterConditionals,
) -> *mut c_char {
    let iter = try_mut_from_ptr!(i);
    iter.next()
        .map(|s| try_ptr_from_str!(s))
        .unwrap_or(ptr::null_mut())
}

/// Free a conditionals iterator.
///
/// # Safety
/// The argument must be a non-null DepSpecIntoIterConditionals pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_dep_set_into_iter_conditionals_free(
    i: *mut DepSpecIntoIterConditionals,
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
