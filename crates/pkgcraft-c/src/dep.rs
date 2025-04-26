use std::cmp::Ordering;
use std::ffi::{c_char, c_int, c_void};
use std::hash::{Hash, Hasher};
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Sub,
    SubAssign,
};
use std::{fmt, ptr, slice};

use pkgcraft::dep::{
    self, Conditionals, Dep, Evaluate, EvaluateForce, Flatten, Recursive, Uri,
};
use pkgcraft::eapi::Eapi;
use pkgcraft::traits::{Contains, IntoOwned};
use pkgcraft::types::Ordered;
use pkgcraft::utils::hash;

use crate::eapi::eapi_or_default;
use crate::error::Error;
use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::types::SetOp;
use crate::utils::boxed;

pub mod cpn;
pub mod cpv;
pub mod pkg;
pub mod uri;
pub mod use_dep;
pub mod version;

/// DependencySet variants.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum DependencySetKind {
    Package,
    SrcUri,
    License,
    Properties,
    RequiredUse,
    Restrict,
}

/// Opaque wrapper for pkgcraft::dep::DependencySet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DependencySetWrapper {
    Dep(dep::DependencySet<Dep>),
    String(dep::DependencySet<String>),
    Uri(dep::DependencySet<Uri>),
}

impl fmt::Display for DependencySetWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Dep(d) => write!(f, "{d}"),
            Self::String(d) => write!(f, "{d}"),
            Self::Uri(d) => write!(f, "{d}"),
        }
    }
}

/// C-compatible wrapper for pkgcraft::dep::DependencySet.
#[derive(Debug)]
#[repr(C)]
pub struct DependencySet {
    set: DependencySetKind,
    dep: *mut DependencySetWrapper,
}

impl Clone for DependencySet {
    fn clone(&self) -> Self {
        let dep = try_ref_from_ptr!(self.dep);
        Self {
            set: self.set,
            dep: Box::into_raw(Box::new(dep.clone())),
        }
    }
}

impl Drop for DependencySet {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.dep));
        }
    }
}

impl DependencySet {
    pub(crate) fn new_dep(d: dep::DependencySet<Dep>) -> Self {
        Self {
            set: DependencySetKind::Package,
            dep: Box::into_raw(Box::new(DependencySetWrapper::Dep(d))),
        }
    }

    pub(crate) fn new_string(d: dep::DependencySet<String>, set: DependencySetKind) -> Self {
        Self {
            set,
            dep: Box::into_raw(Box::new(DependencySetWrapper::String(d))),
        }
    }

    pub(crate) fn new_uri(d: dep::DependencySet<Uri>) -> Self {
        Self {
            set: DependencySetKind::SrcUri,
            dep: Box::into_raw(Box::new(DependencySetWrapper::Uri(d))),
        }
    }
}

impl Hash for DependencySet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl PartialEq for DependencySet {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl Eq for DependencySet {}

impl Deref for DependencySet {
    type Target = DependencySetWrapper;

    fn deref(&self) -> &Self::Target {
        try_ref_from_ptr!(self.dep)
    }
}

impl DerefMut for DependencySet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        try_mut_from_ptr!(self.dep)
    }
}

impl fmt::Display for DependencySet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

impl BitAnd<&DependencySet> for &DependencySet {
    type Output = DependencySet;

    fn bitand(self, other: &DependencySet) -> Self::Output {
        let mut dep = self.clone();
        dep &= other;
        dep
    }
}

impl BitAndAssign<&DependencySet> for DependencySet {
    fn bitand_assign(&mut self, other: &DependencySet) {
        use DependencySetWrapper::*;
        match (self.deref_mut(), other.deref()) {
            (Dep(d1), Dep(d2)) => *d1 &= d2,
            (String(d1), String(d2)) => *d1 &= d2,
            (Uri(d1), Uri(d2)) => *d1 &= d2,
            _ => {
                set_error_and_panic!(Error::new(format!(
                    "DependencySet kind {:?} doesn't match: {:?}",
                    self.set, other.set
                )));
            }
        }
    }
}

impl BitOr<&DependencySet> for &DependencySet {
    type Output = DependencySet;

    fn bitor(self, other: &DependencySet) -> Self::Output {
        let mut dep = self.clone();
        dep |= other;
        dep
    }
}

impl BitOrAssign<&DependencySet> for DependencySet {
    fn bitor_assign(&mut self, other: &DependencySet) {
        use DependencySetWrapper::*;
        match (self.deref_mut(), other.deref()) {
            (Dep(d1), Dep(d2)) => *d1 |= d2,
            (String(d1), String(d2)) => *d1 |= d2,
            (Uri(d1), Uri(d2)) => *d1 |= d2,
            _ => {
                set_error_and_panic!(Error::new(format!(
                    "DependencySet kind {:?} doesn't match: {:?}",
                    self.set, other.set
                )));
            }
        }
    }
}

impl BitXor<&DependencySet> for &DependencySet {
    type Output = DependencySet;

    fn bitxor(self, other: &DependencySet) -> Self::Output {
        let mut dep = self.clone();
        dep ^= other;
        dep
    }
}

impl BitXorAssign<&DependencySet> for DependencySet {
    fn bitxor_assign(&mut self, other: &DependencySet) {
        use DependencySetWrapper::*;
        match (self.deref_mut(), other.deref()) {
            (Dep(d1), Dep(d2)) => *d1 ^= d2,
            (String(d1), String(d2)) => *d1 ^= d2,
            (Uri(d1), Uri(d2)) => *d1 ^= d2,
            _ => {
                set_error_and_panic!(Error::new(format!(
                    "DependencySet kind {:?} doesn't match: {:?}",
                    self.set, other.set
                )));
            }
        }
    }
}

impl Sub<&DependencySet> for &DependencySet {
    type Output = DependencySet;

    fn sub(self, other: &DependencySet) -> Self::Output {
        let mut dep = self.clone();
        dep -= other;
        dep
    }
}

impl SubAssign<&DependencySet> for DependencySet {
    fn sub_assign(&mut self, other: &DependencySet) {
        use DependencySetWrapper::*;
        match (self.deref_mut(), other.deref()) {
            (Dep(d1), Dep(d2)) => *d1 -= d2,
            (String(d1), String(d2)) => *d1 -= d2,
            (Uri(d1), Uri(d2)) => *d1 -= d2,
            _ => {
                set_error_and_panic!(Error::new(format!(
                    "DependencySet kind {:?} doesn't match: {:?}",
                    self.set, other.set
                )));
            }
        }
    }
}

/// Opaque wrapper for pkgcraft::dep::IntoIter<T>.
#[derive(Debug)]
pub enum DependencyIntoIter {
    Dep(DependencySetKind, dep::IntoIter<Dep>),
    String(DependencySetKind, dep::IntoIter<String>),
    Uri(DependencySetKind, dep::IntoIter<Uri>),
}

impl Iterator for DependencyIntoIter {
    type Item = Dependency;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(_, iter) => iter.next().map(Dependency::new_dep),
            Self::String(set, iter) => iter.next().map(|d| Dependency::new_string(d, *set)),
            Self::Uri(_, iter) => iter.next().map(Dependency::new_uri),
        }
    }
}

impl DoubleEndedIterator for DependencyIntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(_, iter) => iter.next_back().map(Dependency::new_dep),
            Self::String(set, iter) => {
                iter.next_back().map(|d| Dependency::new_string(d, *set))
            }
            Self::Uri(_, iter) => iter.next_back().map(Dependency::new_uri),
        }
    }
}

/// Opaque wrapper for pkgcraft::dep::Dependency.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyWrapper {
    Dep(dep::Dependency<Dep>),
    String(dep::Dependency<String>),
    Uri(dep::Dependency<Uri>),
}

impl fmt::Display for DependencyWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Dep(d) => write!(f, "{d}"),
            Self::String(d) => write!(f, "{d}"),
            Self::Uri(d) => write!(f, "{d}"),
        }
    }
}

/// Dependency variants.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub enum DependencyKind {
    Enabled,
    Disabled,
    AllOf,
    AnyOf,
    ExactlyOneOf,
    AtMostOneOf,
    Conditional,
}

impl<T: Ordered> From<&dep::Dependency<T>> for DependencyKind {
    fn from(d: &dep::Dependency<T>) -> Self {
        use dep::Dependency::*;
        match d {
            Enabled(_) => Self::Enabled,
            Disabled(_) => Self::Disabled,
            AllOf(_) => Self::AllOf,
            AnyOf(_) => Self::AnyOf,
            ExactlyOneOf(_) => Self::ExactlyOneOf,
            AtMostOneOf(_) => Self::AtMostOneOf,
            Conditional(_, _) => Self::Conditional,
        }
    }
}

/// C-compatible wrapper for pkgcraft::dep::Dependency.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Dependency {
    set: DependencySetKind,
    kind: DependencyKind,
    dep: *mut DependencyWrapper,
}

impl Drop for Dependency {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.dep));
        }
    }
}

impl Dependency {
    pub(crate) fn new_dep(d: dep::Dependency<Dep>) -> Self {
        Self {
            set: DependencySetKind::Package,
            kind: DependencyKind::from(&d),
            dep: Box::into_raw(Box::new(DependencyWrapper::Dep(d))),
        }
    }

    pub(crate) fn new_string(d: dep::Dependency<String>, set: DependencySetKind) -> Self {
        Self {
            set,
            kind: DependencyKind::from(&d),
            dep: Box::into_raw(Box::new(DependencyWrapper::String(d))),
        }
    }

    pub(crate) fn new_uri(d: dep::Dependency<Uri>) -> Self {
        Self {
            set: DependencySetKind::SrcUri,
            kind: DependencyKind::from(&d),
            dep: Box::into_raw(Box::new(DependencyWrapper::Uri(d))),
        }
    }
}

impl Hash for Dependency {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl Ord for Dependency {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deref().cmp(other.deref())
    }
}

impl PartialOrd for Dependency {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Dependency {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl Eq for Dependency {}

impl Deref for Dependency {
    type Target = DependencyWrapper;

    fn deref(&self) -> &Self::Target {
        try_ref_from_ptr!(self.dep)
    }
}

impl DerefMut for Dependency {
    fn deref_mut(&mut self) -> &mut Self::Target {
        try_mut_from_ptr!(self.dep)
    }
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

/// Opaque wrapper for pkgcraft::dep::IntoIterFlatten<T>.
#[derive(Debug)]
pub enum DependencyIntoIterFlatten {
    Dep(dep::IntoIterFlatten<Dep>),
    String(dep::IntoIterFlatten<String>),
    Uri(dep::IntoIterFlatten<Uri>),
}

impl Iterator for DependencyIntoIterFlatten {
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

/// Opaque wrapper for pkgcraft::dep::IntoIterRecursive<T>.
#[derive(Debug)]
pub enum DependencyIntoIterRecursive {
    Dep(DependencySetKind, dep::IntoIterRecursive<Dep>),
    String(DependencySetKind, dep::IntoIterRecursive<String>),
    Uri(DependencySetKind, dep::IntoIterRecursive<Uri>),
}

impl Iterator for DependencyIntoIterRecursive {
    type Item = Dependency;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(_, iter) => iter.next().map(Dependency::new_dep),
            Self::String(set, iter) => iter.next().map(|d| Dependency::new_string(d, *set)),
            Self::Uri(_, iter) => iter.next().map(Dependency::new_uri),
        }
    }
}

/// Opaque wrapper for pkgcraft::dep::IntoIterConditionals<T>.
#[derive(Debug)]
pub enum DependencyIntoIterConditionals {
    Dep(dep::IntoIterConditionals<Dep>),
    String(dep::IntoIterConditionals<String>),
    Uri(dep::IntoIterConditionals<Uri>),
}

impl Iterator for DependencyIntoIterConditionals {
    type Item = dep::UseDep;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Dep(iter) => iter.next(),
            Self::String(iter) => iter.next(),
            Self::Uri(iter) => iter.next(),
        }
    }
}

/// Create a new, empty DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySetKind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_new(
    kind: DependencySetKind,
) -> *mut DependencySet {
    use DependencySetKind::*;
    let set = match kind {
        Package => DependencySet::new_dep(Default::default()),
        SrcUri => DependencySet::new_uri(Default::default()),
        _ => DependencySet::new_string(Default::default(), kind),
    };

    Box::into_raw(Box::new(set))
}

/// Create a DependencySet from an array of Dependency objects.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be an array of similarly-typed Dependency objects.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_from_iter(
    deps: *mut *mut Dependency,
    len: usize,
    kind: DependencySetKind,
) -> *mut DependencySet {
    ffi_catch_panic! {
        let deps = unsafe { slice::from_raw_parts(deps, len) };
        let deps = deps.iter().map(|p| try_ref_from_ptr!(p));
        let (mut deps_dep, mut deps_string, mut deps_uri) = (vec![], vec![], vec![]);

        for d in deps {
            if d.set != kind {
                set_error_and_panic!(
                    Error::new(format!("Dependency kind {:?} doesn't match: {kind:?}", d.set))
                );
            }

            match d.deref() {
                DependencyWrapper::Dep(d) => deps_dep.push(d.clone()),
                DependencyWrapper::String(d) => deps_string.push(d.clone()),
                DependencyWrapper::Uri(d) => deps_uri.push(d.clone()),
            }
        }

        use DependencySetKind::*;
        let dep = match kind {
            Package => DependencySet::new_dep(deps_dep.into_iter().collect()),
            SrcUri => DependencySet::new_uri(deps_uri.into_iter().collect()),
            _ => DependencySet::new_string(deps_string.into_iter().collect(), kind),
        };

        Box::into_raw(Box::new(dep))
    }
}

/// Parse a string into a specified DependencySet type.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_parse(
    s: *const c_char,
    eapi: *const Eapi,
    kind: DependencySetKind,
) -> *mut DependencySet {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let eapi = eapi_or_default!(eapi);

        use DependencySetKind::*;
        let depset = match kind {
            Package => {
                let opt_dep = unwrap_or_panic!(dep::DependencySet::package(s, eapi));
                DependencySet::new_dep(opt_dep)
            },
            SrcUri => {
                let opt_dep = unwrap_or_panic!(dep::DependencySet::src_uri(s));
                DependencySet::new_uri(opt_dep)
            },
            License => {
                let opt_dep = unwrap_or_panic!(dep::DependencySet::license(s));
                DependencySet::new_string(opt_dep, kind)
            },
            Properties => {
                let opt_dep = unwrap_or_panic!(dep::DependencySet::properties(s));
                DependencySet::new_string(opt_dep, kind)
            },
            RequiredUse => {
                let opt_dep = unwrap_or_panic!(dep::DependencySet::required_use(s));
                DependencySet::new_string(opt_dep, kind)
            },
            Restrict => {
                let opt_dep = unwrap_or_panic!(dep::DependencySet::restrict(s));
                DependencySet::new_string(opt_dep, kind)
            },
        };

        Box::into_raw(Box::new(depset))
    }
}

/// Parse a string into a specified Dependency type.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument should be a UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_parse(
    s: *const c_char,
    eapi: *const Eapi,
    kind: DependencySetKind,
) -> *mut Dependency {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let eapi = eapi_or_default!(eapi);

        use DependencySetKind::*;
        let dep = match kind {
            Package => {
                let dep = unwrap_or_panic!(dep::Dependency::package(s, eapi));
                Dependency::new_dep(dep)
            },
            SrcUri => {
                let dep = unwrap_or_panic!(dep::Dependency::src_uri(s));
                Dependency::new_uri(dep)
            },
            License => {
                let dep = unwrap_or_panic!(dep::Dependency::license(s));
                Dependency::new_string(dep, kind)
            },
            Properties => {
                let dep = unwrap_or_panic!(dep::Dependency::properties(s));
                Dependency::new_string(dep, kind)
            },
            RequiredUse => {
                let dep = unwrap_or_panic!(dep::Dependency::required_use(s));
                Dependency::new_string(dep, kind)
            },
            Restrict => {
                let dep = unwrap_or_panic!(dep::Dependency::restrict(s));
                Dependency::new_string(dep, kind)
            },
        };

        Box::into_raw(Box::new(dep))
    }
}

/// Create a Dependency from a Dep.
///
/// # Safety
/// The argument must be valid Dep pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_from_dep(d: *mut Dep) -> *mut Dependency {
    let d = try_ref_from_ptr!(d);
    let dep = Dependency::new_dep(dep::Dependency::Enabled(d.clone()));
    Box::into_raw(Box::new(dep))
}

/// Evaluate a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_evaluate(
    d: *mut DependencySet,
    options: *mut *mut c_char,
    len: usize,
) -> *mut DependencySet {
    let dep = try_ref_from_ptr!(d);
    let options = unsafe { slice::from_raw_parts(options, len) };
    let options = options.iter().map(|p| try_str_from_ptr!(p)).collect();

    use DependencySetWrapper::*;
    let evaluated = match dep.deref() {
        Dep(d) => Dep(d.evaluate(&options).into_owned()),
        String(d) => String(d.evaluate(&options).into_owned()),
        Uri(d) => Uri(d.evaluate(&options).into_owned()),
    };

    let dep = DependencySet {
        set: dep.set,
        dep: Box::into_raw(Box::new(evaluated)),
    };

    Box::into_raw(Box::new(dep))
}

/// Forcibly evaluate a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_evaluate_force(
    d: *mut DependencySet,
    force: bool,
) -> *mut DependencySet {
    let dep = try_ref_from_ptr!(d);

    use DependencySetWrapper::*;
    let evaluated = match dep.deref() {
        Dep(d) => Dep(d.evaluate_force(force).into_owned()),
        String(d) => String(d.evaluate_force(force).into_owned()),
        Uri(d) => Uri(d.evaluate_force(force).into_owned()),
    };

    let dep = DependencySet {
        set: dep.set,
        dep: Box::into_raw(Box::new(evaluated)),
    };

    Box::into_raw(Box::new(dep))
}

/// Returns true if two DependencySets have no elements in common.
///
/// # Safety
/// The arguments must be a valid DependencySet pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_is_disjoint(
    d1: *mut DependencySet,
    d2: *mut DependencySet,
) -> bool {
    let d1 = try_deref_from_ptr!(d1);
    let d2 = try_deref_from_ptr!(d2);

    use DependencySetWrapper::*;
    match (d1, d2) {
        (Dep(d1), Dep(d2)) => d1.is_disjoint(d2),
        (String(d1), String(d2)) => d1.is_disjoint(d2),
        (Uri(d1), Uri(d2)) => d1.is_disjoint(d2),
        _ => true,
    }
}

/// Returns true if a DependencySet is empty.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_is_empty(d: *mut DependencySet) -> bool {
    let deps = try_deref_from_ptr!(d);

    match deps {
        DependencySetWrapper::Dep(d) => d.is_empty(),
        DependencySetWrapper::String(d) => d.is_empty(),
        DependencySetWrapper::Uri(d) => d.is_empty(),
    }
}

/// Returns true if all the elements of the first DependencySet are contained in the second.
///
/// # Safety
/// The arguments must be a valid DependencySet pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_is_subset(
    d1: *mut DependencySet,
    d2: *mut DependencySet,
) -> bool {
    let d1 = try_deref_from_ptr!(d1);
    let d2 = try_deref_from_ptr!(d2);

    use DependencySetWrapper::*;
    match (d1, d2) {
        (Dep(d1), Dep(d2)) => d1.is_subset(d2),
        (String(d1), String(d2)) => d1.is_subset(d2),
        (Uri(d1), Uri(d2)) => d1.is_subset(d2),
        _ => false,
    }
}

/// Returns the Dependency element for a given index.
///
/// Returns NULL on index nonexistence.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_get_index(
    d: *mut DependencySet,
    index: usize,
) -> *mut Dependency {
    ffi_catch_panic! {
        let set = try_ref_from_ptr!(d);
        let err = || Error::new(format!("failed getting DependencySet index: {index}"));

        use DependencySetWrapper::*;
        let dep = match set.deref() {
            Dep(deps) => {
                deps.get_index(index)
                    .ok_or_else(err)
                    .map(|d| Dependency::new_dep(d.clone()))
            }
            String(deps) => {
                deps.get_index(index)
                    .ok_or_else(err)
                    .map(|d| Dependency::new_string(d.clone(), set.set))
            }
            Uri(deps) => {
                deps.get_index(index)
                    .ok_or_else(err)
                    .map(|d| Dependency::new_uri(d.clone()))
            }
        };

        Box::into_raw(Box::new(unwrap_or_panic!(dep)))
    }
}

/// Sort a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_sort(d: *mut DependencySet) {
    let set = try_mut_from_ptr!(d);

    use DependencySetWrapper::*;
    match set.deref_mut() {
        Dep(deps) => deps.sort(),
        String(deps) => deps.sort(),
        Uri(deps) => deps.sort(),
    }
}

/// Recursively sort a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_sort_recursive(d: *mut DependencySet) {
    let set = try_mut_from_ptr!(d);

    use DependencySetWrapper::*;
    match set.deref_mut() {
        Dep(deps) => deps.sort_recursive(),
        String(deps) => deps.sort_recursive(),
        Uri(deps) => deps.sort_recursive(),
    }
}

/// Clone a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_clone(
    d: *mut DependencySet,
) -> *mut DependencySet {
    let set = try_ref_from_ptr!(d);
    Box::into_raw(Box::new(set.clone()))
}

/// Insert a Dependency into a DependencySet.
///
/// Returns false if an equivalent value already exists, otherwise true.
///
/// # Safety
/// The arguments must be valid DependencySet and Dependency pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_insert(
    d: *mut DependencySet,
    value: *mut Dependency,
) -> bool {
    let set = try_mut_from_ptr!(d);
    let spec = try_deref_from_ptr!(value);

    match (set.deref_mut(), spec.clone()) {
        (DependencySetWrapper::Dep(deps), DependencyWrapper::Dep(dep)) => deps.insert(dep),
        (DependencySetWrapper::String(deps), DependencyWrapper::String(dep)) => {
            deps.insert(dep)
        }
        (DependencySetWrapper::Uri(deps), DependencyWrapper::Uri(dep)) => deps.insert(dep),
        _ => panic!("invalid DependencySet and Dependency type combination"),
    }
}

/// Remove the last value from a DependencySet.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_pop(
    d: *mut DependencySet,
) -> *mut Dependency {
    let set = try_mut_from_ptr!(d);

    use DependencySetWrapper::*;
    let dep = match set.deref_mut() {
        Dep(deps) => deps.pop().map(Dependency::new_dep),
        String(deps) => deps.pop().map(|d| Dependency::new_string(d, set.set)),
        Uri(deps) => deps.pop().map(Dependency::new_uri),
    };

    dep.map(boxed).unwrap_or(ptr::null_mut())
}

/// Replace a Dependency for a given index in a DependencySet, returning the replaced value.
///
/// Returns NULL on index nonexistence or if the DependencySet already contains the given Dependency.
///
/// # Safety
/// The arguments must be valid DependencySet and Dependency pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_replace_index(
    d: *mut DependencySet,
    index: usize,
    value: *mut Dependency,
) -> *mut Dependency {
    let set = try_mut_from_ptr!(d);
    let spec = try_deref_from_ptr!(value);

    let dep = match (set.deref_mut(), spec) {
        (DependencySetWrapper::Dep(deps), DependencyWrapper::Dep(dep)) => deps
            .shift_replace_index(index, dep.clone())
            .map(Dependency::new_dep),
        (DependencySetWrapper::String(deps), DependencyWrapper::String(dep)) => deps
            .shift_replace_index(index, dep.clone())
            .map(|d| Dependency::new_string(d, set.set)),
        (DependencySetWrapper::Uri(deps), DependencyWrapper::Uri(dep)) => deps
            .shift_replace_index(index, dep.clone())
            .map(Dependency::new_uri),
        _ => panic!("invalid DependencySet and Dependency type combination"),
    };

    dep.map(boxed).unwrap_or(ptr::null_mut())
}

/// Replace a Dependency with another Dependency in a DependencySet, returning the replaced value.
///
/// Returns NULL on nonexistence or if the DependencySet already contains the given Dependency.
///
/// # Safety
/// The arguments must be valid DependencySet and Dependency pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_replace(
    d: *mut DependencySet,
    key: *const Dependency,
    value: *mut Dependency,
) -> *mut Dependency {
    let set = try_mut_from_ptr!(d);
    let key = try_deref_from_ptr!(key);
    let value = try_deref_from_ptr!(value);

    let dep = match (set.deref_mut(), key, value) {
        (
            DependencySetWrapper::Dep(deps),
            DependencyWrapper::Dep(k),
            DependencyWrapper::Dep(v),
        ) => deps.shift_replace(k, v.clone()).map(Dependency::new_dep),
        (
            DependencySetWrapper::String(deps),
            DependencyWrapper::String(k),
            DependencyWrapper::String(v),
        ) => deps
            .shift_replace(k, v.clone())
            .map(|d| Dependency::new_string(d, set.set)),
        (
            DependencySetWrapper::Uri(deps),
            DependencyWrapper::Uri(k),
            DependencyWrapper::Uri(v),
        ) => deps.shift_replace(k, v.clone()).map(Dependency::new_uri),
        _ => panic!("invalid DependencySet and Dependency type combination"),
    };

    dep.map(boxed).unwrap_or(ptr::null_mut())
}

/// Perform a set operation on two DependencySets, assigning to the first.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be valid DependencySet pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_assign_op_set(
    op: SetOp,
    d1: *mut DependencySet,
    d2: *mut DependencySet,
) -> *mut DependencySet {
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

/// Perform a set operation on two DependencySets, creating a new set.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be valid DependencySet pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_op_set(
    op: SetOp,
    d1: *mut DependencySet,
    d2: *mut DependencySet,
) -> *mut DependencySet {
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

/// Return the formatted string for a DependencySet object.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_str(d: *mut DependencySet) -> *mut c_char {
    let deps = try_ref_from_ptr!(d);
    try_ptr_from_str!(deps.to_string())
}

/// Determine if two DependencySets are equal.
///
/// # Safety
/// The arguments must be valid DependencySet pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_eq(
    d1: *mut DependencySet,
    d2: *mut DependencySet,
) -> bool {
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);
    d1.eq(d2)
}

/// Determine if a DependencySet contains a given Dependency.
///
/// # Safety
/// The arguments must be valid DependencySet and Dependency pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_contains_dependency(
    s: *mut DependencySet,
    d: *mut Dependency,
) -> bool {
    let s = try_deref_from_ptr!(s);
    let d = try_deref_from_ptr!(d);

    match (s, d) {
        (DependencySetWrapper::Dep(s), DependencyWrapper::Dep(d)) => s.contains(d),
        (DependencySetWrapper::String(s), DependencyWrapper::String(d)) => s.contains(d),
        (DependencySetWrapper::Uri(s), DependencyWrapper::Uri(d)) => s.contains(d),
        _ => false,
    }
}

/// Determine if a DependencySet contains a given raw string.
///
/// # Safety
/// The arguments must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_contains_str(
    d: *mut DependencySet,
    s: *const c_char,
) -> bool {
    let d = try_deref_from_ptr!(d);
    let s = try_str_from_ptr!(s);

    match d {
        DependencySetWrapper::Dep(d) => d.contains(s),
        DependencySetWrapper::String(d) => d.contains(s),
        DependencySetWrapper::Uri(d) => d.contains(s),
    }
}

/// Determine if a DependencySet contains a given UseDep.
///
/// # Safety
/// The arguments must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_contains_use_dep(
    d: *mut DependencySet,
    u: *mut use_dep::UseDep,
) -> bool {
    let d = try_deref_from_ptr!(d);
    let u = try_deref_from_ptr!(u);

    match d {
        DependencySetWrapper::Dep(d) => d.contains(u),
        DependencySetWrapper::String(d) => d.contains(u),
        DependencySetWrapper::Uri(d) => d.contains(u),
    }
}

/// Determine if a DependencySet contains a given Dep.
///
/// # Safety
/// The arguments must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_contains_dep(
    d: *mut DependencySet,
    dep: *mut Dep,
) -> bool {
    let d = try_deref_from_ptr!(d);
    let dep = try_ref_from_ptr!(dep);

    match d {
        DependencySetWrapper::Dep(d) => d.contains(dep),
        _ => false,
    }
}

/// Determine if a DependencySet contains a given Uri.
///
/// # Safety
/// The arguments must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_contains_uri(
    d: *mut DependencySet,
    uri: *mut Uri,
) -> bool {
    let d = try_deref_from_ptr!(d);
    let uri = try_ref_from_ptr!(uri);

    match d {
        DependencySetWrapper::Uri(d) => d.contains(uri),
        _ => false,
    }
}

/// Return the hash value for a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_hash(d: *mut DependencySet) -> u64 {
    let deps = try_ref_from_ptr!(d);
    hash(deps)
}

/// Return a DependencySet's length.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_len(d: *mut DependencySet) -> usize {
    let deps = try_deref_from_ptr!(d);
    use DependencySetWrapper::*;
    match deps {
        Dep(d) => d.len(),
        String(d) => d.len(),
        Uri(d) => d.len(),
    }
}

/// Return an iterator for a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter(
    d: *mut DependencySet,
) -> *mut DependencyIntoIter {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DependencySetWrapper::Dep(d) => DependencyIntoIter::Dep(deps.set, d.into_iter()),
        DependencySetWrapper::String(d) => DependencyIntoIter::String(deps.set, d.into_iter()),
        DependencySetWrapper::Uri(d) => DependencyIntoIter::Uri(deps.set, d.into_iter()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a DependencySet iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a valid DependencyIntoIter pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_next(
    i: *mut DependencyIntoIter,
) -> *mut Dependency {
    let iter = try_mut_from_ptr!(i);
    iter.next().map(boxed).unwrap_or(ptr::null_mut())
}

/// Return the next object from the end of a DependencySet iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a valid DependencyIntoIter pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_next_back(
    i: *mut DependencyIntoIter,
) -> *mut Dependency {
    let iter = try_mut_from_ptr!(i);
    iter.next_back().map(boxed).unwrap_or(ptr::null_mut())
}

/// Free a DependencySet iterator.
///
/// # Safety
/// The argument must be a valid DependencyIntoIter pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_free(i: *mut DependencyIntoIter) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Evaluate a Dependency.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_evaluate(
    d: *mut Dependency,
    options: *mut *mut c_char,
    len: usize,
    deps_len: *mut usize,
) -> *mut *mut Dependency {
    let dep = try_ref_from_ptr!(d);
    let options = unsafe { slice::from_raw_parts(options, len) };
    let options = options.iter().map(|p| try_str_from_ptr!(p)).collect();

    use DependencyWrapper::*;
    match dep.deref() {
        Dep(d) => {
            iter_to_array!(d.evaluate(&options).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(Dependency::new_dep(d.into_owned())))
            })
        }
        String(d) => {
            iter_to_array!(d.evaluate(&options).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(Dependency::new_string(d.into_owned(), dep.set)))
            })
        }
        Uri(d) => {
            iter_to_array!(d.evaluate(&options).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(Dependency::new_uri(d.into_owned())))
            })
        }
    }
}

/// Forcibly evaluate a Dependency.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_evaluate_force(
    d: *mut Dependency,
    force: bool,
    deps_len: *mut usize,
) -> *mut *mut Dependency {
    let dep = try_ref_from_ptr!(d);

    use DependencyWrapper::*;
    match dep.deref() {
        Dep(d) => {
            iter_to_array!(d.evaluate_force(force).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(Dependency::new_dep(d.into_owned())))
            })
        }
        String(d) => {
            iter_to_array!(d.evaluate_force(force).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(Dependency::new_string(d.into_owned(), dep.set)))
            })
        }
        Uri(d) => {
            iter_to_array!(d.evaluate_force(force).into_iter(), deps_len, |d| {
                Box::into_raw(Box::new(Dependency::new_uri(d.into_owned())))
            })
        }
    }
}

/// Return the conditional for a Dependency.
///
/// Returns NULL if the Dependency variant isn't conditional.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_conditional(
    d: *mut Dependency,
) -> *mut use_dep::UseDep {
    let d = try_deref_from_ptr!(d);

    use DependencyWrapper::*;
    let use_dep = match d {
        Dep(dep::Dependency::Conditional(u, _)) => Some(u.clone().into()),
        String(dep::Dependency::Conditional(u, _)) => Some(u.clone().into()),
        Uri(dep::Dependency::Conditional(u, _)) => Some(u.clone().into()),
        _ => None,
    };

    use_dep.map(boxed).unwrap_or(ptr::null_mut())
}

/// Compare two Dependencys returning -1, 0, or 1 if the first is less than, equal to, or greater
/// than the second, respectively.
///
/// # Safety
/// The arguments must be valid Dependency pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_cmp(
    d1: *mut Dependency,
    d2: *mut Dependency,
) -> c_int {
    let d1 = try_ref_from_ptr!(d1);
    let d2 = try_ref_from_ptr!(d2);

    match d1.cmp(d2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Determine if a Dependency contains a given Dependency.
///
/// # Safety
/// The arguments must be valid Dependency pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_contains_dependency(
    d1: *mut Dependency,
    d2: *mut Dependency,
) -> bool {
    let d1 = try_deref_from_ptr!(d1);
    let d2 = try_deref_from_ptr!(d2);

    use DependencyWrapper::*;
    match (d1, d2) {
        (Dep(d1), Dep(d2)) => d1.contains(d2),
        (String(d1), String(d2)) => d1.contains(d2),
        (Uri(d1), Uri(d2)) => d1.contains(d2),
        _ => false,
    }
}

/// Determine if a Dependency contains a given raw string.
///
/// # Safety
/// The arguments must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_contains_str(
    d: *mut Dependency,
    s: *const c_char,
) -> bool {
    let d = try_deref_from_ptr!(d);
    let s = try_str_from_ptr!(s);

    match d {
        DependencyWrapper::Dep(d) => d.contains(s),
        DependencyWrapper::String(d) => d.contains(s),
        DependencyWrapper::Uri(d) => d.contains(s),
    }
}

/// Determine if a Dependency contains a given UseDep.
///
/// # Safety
/// The arguments must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_contains_use_dep(
    d: *mut Dependency,
    u: *mut use_dep::UseDep,
) -> bool {
    let d = try_deref_from_ptr!(d);
    let u = try_deref_from_ptr!(u);

    match d {
        DependencyWrapper::Dep(d) => d.contains(u),
        DependencyWrapper::String(d) => d.contains(u),
        DependencyWrapper::Uri(d) => d.contains(u),
    }
}

/// Determine if a Dependency contains a given Dep.
///
/// # Safety
/// The arguments must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_contains_dep(
    d: *mut Dependency,
    dep: *mut Dep,
) -> bool {
    let d = try_deref_from_ptr!(d);
    let dep = try_ref_from_ptr!(dep);

    match d {
        DependencyWrapper::Dep(d) => d.contains(dep),
        _ => false,
    }
}

/// Determine if a Dependency contains a given Uri.
///
/// # Safety
/// The arguments must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_contains_uri(
    d: *mut Dependency,
    uri: *mut Uri,
) -> bool {
    let d = try_deref_from_ptr!(d);
    let uri = try_ref_from_ptr!(uri);

    match d {
        DependencyWrapper::Uri(d) => d.contains(uri),
        _ => false,
    }
}

/// Return the Dependency for a given index if it exists.
///
/// Returns NULL on index nonexistence.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_get_index(
    d: *mut Dependency,
    index: usize,
) -> *mut Dependency {
    ffi_catch_panic! {
        let dep = try_ref_from_ptr!(d);
        let err = || Error::new(format!("failed getting Dependency index: {index}"));

        use DependencyWrapper::*;
        let dep = match dep.deref() {
            Dep(d) => {
                d.get_index(index)
                    .ok_or_else(err)
                    .map(|d| Dependency::new_dep(d.clone()))
            }
            String(d) => {
                d.get_index(index)
                    .ok_or_else(err)
                    .map(|d| Dependency::new_string(d.clone(), dep.set))
            }
            Uri(d) => {
                d.get_index(index)
                    .ok_or_else(err)
                    .map(|d| Dependency::new_uri(d.clone()))
            }
        };

        Box::into_raw(Box::new(unwrap_or_panic!(dep)))
    }
}

/// Return the hash value for a Dependency.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_hash(d: *mut Dependency) -> u64 {
    let deps = try_ref_from_ptr!(d);
    hash(deps)
}

/// Return a Dependency's length.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_len(d: *mut Dependency) -> usize {
    let deps = try_deref_from_ptr!(d);
    use DependencyWrapper::*;
    match deps {
        Dep(d) => d.len(),
        String(d) => d.len(),
        Uri(d) => d.len(),
    }
}

/// Recursively sort a Dependency.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_sort(d: *mut Dependency) {
    let dep = try_mut_from_ptr!(d);

    use DependencyWrapper::*;
    match dep.deref_mut() {
        Dep(d) => d.sort(),
        String(d) => d.sort(),
        Uri(d) => d.sort(),
    }
}

/// Return the formatted string for a Dependency object.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_str(d: *mut Dependency) -> *mut c_char {
    let deps = try_ref_from_ptr!(d);
    try_ptr_from_str!(deps.to_string())
}

/// Free a Dependency object.
///
/// # Safety
/// The argument must be a Dependency pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_free(r: *mut Dependency) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}

/// Return an iterator for a Dependency.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_into_iter(
    d: *mut Dependency,
) -> *mut DependencyIntoIter {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DependencyWrapper::Dep(d) => DependencyIntoIter::Dep(deps.set, d.into_iter()),
        DependencyWrapper::String(d) => DependencyIntoIter::String(deps.set, d.into_iter()),
        DependencyWrapper::Uri(d) => DependencyIntoIter::Uri(deps.set, d.into_iter()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return a flatten iterator for a Dependency.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_into_iter_flatten(
    d: *mut Dependency,
) -> *mut DependencyIntoIterFlatten {
    let dep = try_deref_from_ptr!(d);
    let iter = match dep.clone() {
        DependencyWrapper::Dep(d) => DependencyIntoIterFlatten::Dep(d.into_iter_flatten()),
        DependencyWrapper::String(d) => {
            DependencyIntoIterFlatten::String(d.into_iter_flatten())
        }
        DependencyWrapper::Uri(d) => DependencyIntoIterFlatten::Uri(d.into_iter_flatten()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return a flatten iterator for a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_flatten(
    d: *mut DependencySet,
) -> *mut DependencyIntoIterFlatten {
    let deps = try_deref_from_ptr!(d);
    let iter = match deps.clone() {
        DependencySetWrapper::Dep(d) => DependencyIntoIterFlatten::Dep(d.into_iter_flatten()),
        DependencySetWrapper::String(d) => {
            DependencyIntoIterFlatten::String(d.into_iter_flatten())
        }
        DependencySetWrapper::Uri(d) => DependencyIntoIterFlatten::Uri(d.into_iter_flatten()),
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a flatten iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a valid DependencyIntoIterFlatten pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_flatten_next(
    i: *mut DependencyIntoIterFlatten,
) -> *mut c_void {
    let iter = try_mut_from_ptr!(i);
    iter.next().unwrap_or(ptr::null_mut())
}

/// Free a flatten iterator.
///
/// # Safety
/// The argument must be a valid DependencyIntoIterFlatten pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_flatten_free(
    i: *mut DependencyIntoIterFlatten,
) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Return a recursive iterator for a Dependency.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_into_iter_recursive(
    d: *mut Dependency,
) -> *mut DependencyIntoIterRecursive {
    let dep = try_ref_from_ptr!(d);
    let iter = match dep.deref().clone() {
        DependencyWrapper::Dep(d) => {
            DependencyIntoIterRecursive::Dep(dep.set, d.into_iter_recursive())
        }
        DependencyWrapper::String(d) => {
            DependencyIntoIterRecursive::String(dep.set, d.into_iter_recursive())
        }
        DependencyWrapper::Uri(d) => {
            DependencyIntoIterRecursive::Uri(dep.set, d.into_iter_recursive())
        }
    };
    Box::into_raw(Box::new(iter))
}

/// Return a recursive iterator for a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_recursive(
    d: *mut DependencySet,
) -> *mut DependencyIntoIterRecursive {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DependencySetWrapper::Dep(d) => {
            DependencyIntoIterRecursive::Dep(deps.set, d.into_iter_recursive())
        }
        DependencySetWrapper::String(d) => {
            DependencyIntoIterRecursive::String(deps.set, d.into_iter_recursive())
        }
        DependencySetWrapper::Uri(d) => {
            DependencyIntoIterRecursive::Uri(deps.set, d.into_iter_recursive())
        }
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a recursive iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a valid DependencyIntoIterRecursive pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_recursive_next(
    i: *mut DependencyIntoIterRecursive,
) -> *mut Dependency {
    let iter = try_mut_from_ptr!(i);
    iter.next().map(boxed).unwrap_or(ptr::null_mut())
}

/// Free a recursive iterator.
///
/// # Safety
/// The argument must be a valid DependencyIntoIterRecursive pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_recursive_free(
    i: *mut DependencyIntoIterRecursive,
) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Return a conditionals iterator for a Dependency.
///
/// # Safety
/// The argument must be a valid Dependency pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_into_iter_conditionals(
    d: *mut Dependency,
) -> *mut DependencyIntoIterConditionals {
    let dep = try_ref_from_ptr!(d);
    let iter = match dep.deref().clone() {
        DependencyWrapper::Dep(d) => {
            DependencyIntoIterConditionals::Dep(d.into_iter_conditionals())
        }
        DependencyWrapper::String(d) => {
            DependencyIntoIterConditionals::String(d.into_iter_conditionals())
        }
        DependencyWrapper::Uri(d) => {
            DependencyIntoIterConditionals::Uri(d.into_iter_conditionals())
        }
    };
    Box::into_raw(Box::new(iter))
}

/// Return a conditionals iterator for a DependencySet.
///
/// # Safety
/// The argument must be a valid DependencySet pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_conditionals(
    d: *mut DependencySet,
) -> *mut DependencyIntoIterConditionals {
    let deps = try_ref_from_ptr!(d);
    let iter = match deps.deref().clone() {
        DependencySetWrapper::Dep(d) => {
            DependencyIntoIterConditionals::Dep(d.into_iter_conditionals())
        }
        DependencySetWrapper::String(d) => {
            DependencyIntoIterConditionals::String(d.into_iter_conditionals())
        }
        DependencySetWrapper::Uri(d) => {
            DependencyIntoIterConditionals::Uri(d.into_iter_conditionals())
        }
    };
    Box::into_raw(Box::new(iter))
}

/// Return the next object from a conditionals iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a valid DependencyIntoIterConditionals pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_conditionals_next(
    i: *mut DependencyIntoIterConditionals,
) -> *mut use_dep::UseDep {
    let iter = try_mut_from_ptr!(i);
    iter.next()
        .map(|x| boxed(x.into()))
        .unwrap_or(ptr::null_mut())
}

/// Free a conditionals iterator.
///
/// # Safety
/// The argument must be a valid DependencyIntoIterConditionals pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_into_iter_conditionals_free(
    i: *mut DependencyIntoIterConditionals,
) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Free a DependencySet.
///
/// # Safety
/// The argument must be a DependencySet pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_dependency_set_free(d: *mut DependencySet) {
    if !d.is_null() {
        unsafe { drop(Box::from_raw(d)) };
    }
}
