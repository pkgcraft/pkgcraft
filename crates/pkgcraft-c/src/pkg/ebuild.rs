use std::ffi::{c_char, CString};
use std::{mem, ptr, slice};

use pkgcraft::pkg::ebuild::xml;
use pkgcraft::pkg::Pkg;
use pkgcraft::traits::IntoOwned;

use crate::dep::{DependencySet, DependencySetKind};
use crate::error::Error;
use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::utils::{boxed, obj_to_str, str_to_raw};

pub mod keyword;
use keyword::Keyword;

/// Wrapper for package maintainers.
#[repr(C)]
pub struct Maintainer {
    email: *mut c_char,
    name: *mut c_char,
    description: *mut c_char,
    maint_type: *mut c_char,
    proxied: *mut c_char,
}

impl Drop for Maintainer {
    fn drop(&mut self) {
        unsafe {
            drop(CString::from_raw(self.email));
            char_p_or_null_free!(self.name);
            char_p_or_null_free!(self.description);
            drop(CString::from_raw(self.maint_type));
            drop(CString::from_raw(self.proxied));
        }
    }
}

/// Wrapper for package upstream remote-ids.
#[repr(C)]
pub struct RemoteId {
    site: *mut c_char,
    name: *mut c_char,
}

impl Drop for RemoteId {
    fn drop(&mut self) {
        unsafe {
            drop(CString::from_raw(self.site));
            drop(CString::from_raw(self.name));
        }
    }
}

/// Wrapper for upstream package maintainers.
#[repr(C)]
pub struct UpstreamMaintainer {
    name: *mut c_char,
    email: *mut c_char,
    status: *mut c_char,
}

impl Drop for UpstreamMaintainer {
    fn drop(&mut self) {
        unsafe {
            drop(CString::from_raw(self.name));
            char_p_or_null_free!(self.email);
            drop(CString::from_raw(self.status));
        }
    }
}

/// Wrapper for package upstream info.
#[repr(C)]
pub struct Upstream {
    remote_ids_len: usize,
    remote_ids: *mut *mut RemoteId,
    maintainers_len: usize,
    maintainers: *mut *mut UpstreamMaintainer,
    bugs_to: *mut c_char,
    changelog: *mut c_char,
    doc: *mut c_char,
}

impl Drop for Upstream {
    fn drop(&mut self) {
        unsafe {
            let len = self.remote_ids_len;
            for ptr in Vec::from_raw_parts(self.remote_ids, len, len).into_iter() {
                drop(Box::from_raw(ptr));
            }
            let len = self.maintainers_len;
            for ptr in Vec::from_raw_parts(self.maintainers, len, len).into_iter() {
                drop(Box::from_raw(ptr));
            }
            char_p_or_null_free!(self.bugs_to);
            char_p_or_null_free!(self.changelog);
            char_p_or_null_free!(self.doc);
        }
    }
}

/// Convert a given pointer into an ebuild package reference.
macro_rules! try_pkg_from_ptr {
    ( $var:expr ) => {{
        let pkg = $crate::macros::try_ref_from_ptr!($var);
        match pkg {
            Pkg::Ebuild(p) => p,
            Pkg::Configured(p) => p.into(),
            _ => panic!("invalid pkg type: {pkg:?}"),
        }
    }};
}

/// Return a package's path.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_path(p: *mut Pkg) -> *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    try_ptr_from_str!(pkg.path().as_str())
}

/// Return a package's ebuild file content.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_data(p: *mut Pkg) -> *mut c_char {
    ffi_catch_panic! {
        let pkg = try_pkg_from_ptr!(p);
        try_ptr_from_str!(pkg.data())
    }
}

/// Return a package's deprecated status.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_deprecated(p: *mut Pkg) -> bool {
    let pkg = try_pkg_from_ptr!(p);
    pkg.deprecated()
}

/// Return a package's live status.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_live(p: *mut Pkg) -> bool {
    let pkg = try_pkg_from_ptr!(p);
    pkg.live()
}

/// Return a package's masked status.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_masked(p: *mut Pkg) -> bool {
    let pkg = try_pkg_from_ptr!(p);
    pkg.masked()
}

/// Return a package's description.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_description(p: *mut Pkg) -> *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    try_ptr_from_str!(pkg.description())
}

/// Return a package's slot.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_slot(p: *mut Pkg) -> *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    try_ptr_from_str!(pkg.slot())
}

/// Return a package's subslot.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_subslot(p: *mut Pkg) -> *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    try_ptr_from_str!(pkg.subslot())
}

/// Return a package's dependencies for a given set of descriptors.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_dependencies(
    p: *mut Pkg,
    keys: *mut *mut c_char,
    len: usize,
) -> *mut DependencySet {
    ffi_catch_panic! {
        let pkg = try_pkg_from_ptr!(p);
        let keys = unsafe { slice::from_raw_parts(keys, len) };
        let mut dep_keys = vec![];
        for s in keys {
            let s = try_str_from_ptr!(s);
            let key = unwrap_or_panic!(
                s.to_uppercase().parse().map_err(|_| Error::new(format!("invalid dep key: {s}")))
            );
            dep_keys.push(key);
        }

        let deps = pkg.dependencies(dep_keys).into_owned();
        Box::into_raw(Box::new(DependencySet::new_dep(deps)))
    }
}

/// Return a package's DEPEND.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_depend(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set = DependencySet::new_dep(pkg.depend().clone());
    Box::into_raw(Box::new(set))
}

/// Return a package's BDEPEND.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_bdepend(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set = DependencySet::new_dep(pkg.bdepend().clone());
    Box::into_raw(Box::new(set))
}

/// Return a package's IDEPEND.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_idepend(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set = DependencySet::new_dep(pkg.idepend().clone());
    Box::into_raw(Box::new(set))
}

/// Return a package's PDEPEND.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_pdepend(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set = DependencySet::new_dep(pkg.pdepend().clone());
    Box::into_raw(Box::new(set))
}

/// Return a package's RDEPEND.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_rdepend(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set = DependencySet::new_dep(pkg.rdepend().clone());
    Box::into_raw(Box::new(set))
}

/// Return a package's LICENSE.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_license(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set = DependencySet::new_string(pkg.license().clone(), DependencySetKind::License);
    Box::into_raw(Box::new(set))
}

/// Return a package's PROPERTIES.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_properties(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set =
        DependencySet::new_string(pkg.properties().clone(), DependencySetKind::Properties);
    Box::into_raw(Box::new(set))
}

/// Return a package's REQUIRED_USE.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_required_use(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set =
        DependencySet::new_string(pkg.required_use().clone(), DependencySetKind::RequiredUse);
    Box::into_raw(Box::new(set))
}

/// Return a package's RESTRICT.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_restrict(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set = DependencySet::new_string(pkg.restrict().clone(), DependencySetKind::Restrict);
    Box::into_raw(Box::new(set))
}

/// Return a package's SRC_URI.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_src_uri(p: *mut Pkg) -> *mut DependencySet {
    let pkg = try_pkg_from_ptr!(p);
    let set = DependencySet::new_uri(pkg.src_uri().clone());
    Box::into_raw(Box::new(set))
}

/// Return a package's homepage.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_homepage(
    p: *mut Pkg,
    len: *mut usize,
) -> *mut *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    iter_to_array!(pkg.homepage().iter(), len, str_to_raw)
}

/// Return a package's defined phases.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_defined_phases(
    p: *mut Pkg,
    len: *mut usize,
) -> *mut *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    iter_to_array!(pkg.defined_phases().iter(), len, str_to_raw)
}

/// Return a package's keywords.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_keywords(
    p: *mut Pkg,
    len: *mut usize,
) -> *mut *mut Keyword {
    let pkg = try_pkg_from_ptr!(p);
    iter_to_array!(pkg.keywords().iter(), len, |x| boxed(x.clone().into()))
}

/// Return a package's keywords as raw strings.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_keywords_str(
    p: *mut Pkg,
    len: *mut usize,
) -> *mut *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    iter_to_array!(pkg.keywords().iter(), len, obj_to_str)
}

/// Return a package's iuse.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_iuse(
    p: *mut Pkg,
    len: *mut usize,
) -> *mut *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    iter_to_array!(pkg.iuse().iter(), len, obj_to_str)
}

/// Return a package's directly inherited eclasses.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_inherit(
    p: *mut Pkg,
    len: *mut usize,
) -> *mut *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    iter_to_array!(pkg.inherit().iter().map(|e| e.name()), len, str_to_raw)
}

/// Return a package's inherited eclasses.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_inherited(
    p: *mut Pkg,
    len: *mut usize,
) -> *mut *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    iter_to_array!(pkg.inherited().iter().map(|e| e.name()), len, str_to_raw)
}

/// Return a package's long description.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_long_description(p: *mut Pkg) -> *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.metadata().description() {
        Some(s) => try_ptr_from_str!(s),
        None => ptr::null_mut(),
    }
}

/// Return a package's maintainers.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_maintainers(
    p: *mut Pkg,
    len: *mut usize,
) -> *mut *mut Maintainer {
    let pkg = try_pkg_from_ptr!(p);
    let mut ptrs: Vec<_> = pkg
        .metadata()
        .maintainers()
        .iter()
        .map(|m| {
            let maintainer = Maintainer {
                email: try_ptr_from_str!(m.email()),
                name: char_p_or_null!(m.name()),
                description: char_p_or_null!(m.description()),
                maint_type: try_ptr_from_str!(m.maint_type().as_ref()),
                proxied: try_ptr_from_str!(m.proxied().as_ref()),
            };
            Box::into_raw(Box::new(maintainer))
        })
        .collect();
    ptrs.shrink_to_fit();
    unsafe { *len = ptrs.len() };
    let p = ptrs.as_mut_ptr();
    mem::forget(ptrs);
    p
}

/// Free an array of Maintainer pointers.
///
/// # Safety
/// The argument must be the value received from pkgcraft_pkg_ebuild_maintainers() or NULL along
/// with the length of the array.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_maintainers_free(
    maintainers: *mut *mut Maintainer,
    len: usize,
) {
    if !maintainers.is_null() {
        unsafe {
            for ptr in Vec::from_raw_parts(maintainers, len, len).into_iter() {
                drop(Box::from_raw(ptr));
            }
        }
    }
}

/// Return a package's upstream info.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_upstream(p: *mut Pkg) -> *mut Upstream {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.metadata().upstream() {
        Some(u) => {
            // convert remote ids to C wrapper objects
            let mut remote_ids_len: usize = 0;
            let convert = |r: &xml::RemoteId| {
                let obj = RemoteId {
                    site: try_ptr_from_str!(r.site()),
                    name: try_ptr_from_str!(r.name()),
                };
                Box::into_raw(Box::new(obj))
            };
            let remote_ids =
                iter_to_array!(u.remote_ids().iter(), &mut remote_ids_len as *mut _, convert);

            // convert upstream maintainers to C wrapper objects
            let mut maintainers_len: usize = 0;
            let convert = |m: &xml::UpstreamMaintainer| {
                let obj = UpstreamMaintainer {
                    name: try_ptr_from_str!(m.name()),
                    email: char_p_or_null!(m.email()),
                    status: try_ptr_from_str!(m.status().to_string()),
                };
                Box::into_raw(Box::new(obj))
            };
            let maintainers = iter_to_array!(
                u.maintainers().iter(),
                &mut maintainers_len as *mut _,
                convert
            );

            let upstream = Upstream {
                remote_ids_len,
                remote_ids,
                maintainers_len,
                maintainers,
                bugs_to: char_p_or_null!(u.bugs_to()),
                changelog: char_p_or_null!(u.changelog()),
                doc: char_p_or_null!(u.doc()),
            };

            Box::into_raw(Box::new(upstream))
        }
        None => ptr::null_mut(),
    }
}

/// Free an Upstream.
///
/// # Safety
/// The argument must be a Upstream pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_upstream_free(u: *mut Upstream) {
    if !u.is_null() {
        unsafe { drop(Box::from_raw(u)) };
    }
}
