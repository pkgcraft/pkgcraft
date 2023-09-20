use std::ffi::{c_char, CString};
use std::str::FromStr;
use std::{mem, ptr, slice};

use pkgcraft::pkg::ebuild::metadata;
use pkgcraft::pkg::Pkg;
use pkgcraft::shell::Key;

use crate::dep::spec::{DepSet, DepSetKind};
use crate::error::Error;
use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::utils::str_to_raw;

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
        match pkg.as_ebuild() {
            Some((p, _)) => p,
            None => panic!("invalid pkg type: {pkg:?}"),
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
    try_ptr_from_str!(pkg.abspath().as_str())
}

/// Return a package's ebuild file content.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_ebuild(p: *mut Pkg) -> *mut c_char {
    ffi_catch_panic! {
        let pkg = try_pkg_from_ptr!(p);
        let s = unwrap_or_panic!(pkg.ebuild());
        try_ptr_from_str!(s)
    }
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
) -> *mut DepSet {
    ffi_catch_panic! {
        let pkg = try_pkg_from_ptr!(p);
        let keys = unsafe { slice::from_raw_parts(keys, len) };
        let mut dep_keys = vec![];
        for s in keys {
            let s = try_str_from_ptr!(s).to_uppercase();
            let key = unwrap_or_panic!(
                Key::from_str(&s).map_err(|_| Error::new(format!("invalid dep key: {s}")))
            );
            dep_keys.push(key);
        }

        let deps = pkg.dependencies(&dep_keys);
        Box::into_raw(Box::new(DepSet::new_dep(deps)))
    }
}

/// Return a package's DEPEND.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_depend(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.depend() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_dep(d.clone()))),
        None => ptr::null_mut(),
    }
}

/// Return a package's BDEPEND.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_bdepend(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.bdepend() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_dep(d.clone()))),
        None => ptr::null_mut(),
    }
}

/// Return a package's IDEPEND.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_idepend(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.idepend() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_dep(d.clone()))),
        None => ptr::null_mut(),
    }
}

/// Return a package's PDEPEND.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_pdepend(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.pdepend() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_dep(d.clone()))),
        None => ptr::null_mut(),
    }
}

/// Return a package's RDEPEND.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_rdepend(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.rdepend() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_dep(d.clone()))),
        None => ptr::null_mut(),
    }
}

/// Return a package's LICENSE.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_license(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.license() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_string(d.clone(), DepSetKind::License))),
        None => ptr::null_mut(),
    }
}

/// Return a package's PROPERTIES.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_properties(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.properties() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_string(d.clone(), DepSetKind::Properties))),
        None => ptr::null_mut(),
    }
}

/// Return a package's REQUIRED_USE.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_required_use(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.required_use() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_string(d.clone(), DepSetKind::RequiredUse))),
        None => ptr::null_mut(),
    }
}

/// Return a package's RESTRICT.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_restrict(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.restrict() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_string(d.clone(), DepSetKind::Restrict))),
        None => ptr::null_mut(),
    }
}

/// Return a package's SRC_URI.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_src_uri(p: *mut Pkg) -> *mut DepSet {
    let pkg = try_pkg_from_ptr!(p);
    match pkg.src_uri() {
        Some(d) => Box::into_raw(Box::new(DepSet::new_uri(d.clone()))),
        None => ptr::null_mut(),
    }
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
) -> *mut *mut c_char {
    let pkg = try_pkg_from_ptr!(p);
    iter_to_array!(pkg.keywords().iter(), len, str_to_raw)
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
    iter_to_array!(pkg.iuse().iter(), len, str_to_raw)
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
    iter_to_array!(pkg.inherit().iter(), len, str_to_raw)
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
    iter_to_array!(pkg.inherited().iter(), len, str_to_raw)
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
    match pkg.xml().long_description() {
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
        .xml()
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
    match pkg.xml().upstream() {
        Some(u) => {
            // convert remote ids to C wrapper objects
            let mut remote_ids_len: usize = 0;
            let convert = |r: &metadata::RemoteId| {
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
            let convert = |m: &metadata::UpstreamMaintainer| {
                let obj = UpstreamMaintainer {
                    name: try_ptr_from_str!(m.name()),
                    email: char_p_or_null!(m.email()),
                    status: try_ptr_from_str!(m.status().to_string()),
                };
                Box::into_raw(Box::new(obj))
            };
            let maintainers =
                iter_to_array!(u.maintainers().iter(), &mut maintainers_len as *mut _, convert);

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
