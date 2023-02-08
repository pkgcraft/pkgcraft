use std::ffi::{c_char, CStr, CString};
use std::str::FromStr;
use std::{mem, ptr, slice};

use pkgcraft::pkg::Pkg;
use pkgcraft::pkgsh::Key;

use crate::depset::DepSet;
use crate::error::Error;
use crate::macros::*;
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
            char_p_or_null_free!(self.maint_type);
            char_p_or_null_free!(self.proxied);
        }
    }
}

/// Wrapper for package upstreams.
#[repr(C)]
pub struct Upstream {
    site: *mut c_char,
    name: *mut c_char,
}

impl Drop for Upstream {
    fn drop(&mut self) {
        unsafe {
            drop(CString::from_raw(self.site));
            drop(CString::from_raw(self.name));
        }
    }
}

/// Return a package's path.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_path(p: *mut Pkg) -> *mut c_char {
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    CString::new(pkg.path().as_str()).unwrap().into_raw()
}

/// Return a package's ebuild file content.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_ebuild(p: *mut Pkg) -> *mut c_char {
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    let s = unwrap_or_return!(pkg.ebuild(), ptr::null_mut());
    let cstring = unwrap_or_return!(
        CString::new(s).map_err(|e| Error::new(format!("invalid ebuild file data: {e}"))),
        ptr::null_mut()
    );
    cstring.into_raw()
}

/// Return a package's description.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_description(p: *mut Pkg) -> *mut c_char {
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    CString::new(pkg.description()).unwrap().into_raw()
}

/// Return a package's slot.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_slot(p: *mut Pkg) -> *mut c_char {
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    CString::new(pkg.slot()).unwrap().into_raw()
}

/// Return a package's subslot.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_subslot(p: *mut Pkg) -> *mut c_char {
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    CString::new(pkg.subslot()).unwrap().into_raw()
}

/// Return a package's dependencies for a given set of descriptors.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_dependencies(
    p: *mut Pkg,
    keys: *mut *mut c_char,
    len: usize,
) -> *mut DepSet {
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");

    let keys = unsafe { slice::from_raw_parts(keys, len) };
    let mut dep_keys = Vec::<Key>::new();
    for s in keys {
        let s = null_ptr_check!(s.as_ref());
        let s = unsafe { unwrap_or_return!(CStr::from_ptr(s).to_str(), ptr::null_mut()) };
        let key = unwrap_or_return!(
            Key::from_str(s).map_err(|_| Error::new(format!("invalid dep key: {s}"))),
            ptr::null_mut()
        );
        dep_keys.push(key);
    }

    let deps = pkg.dependencies(&dep_keys);
    Box::into_raw(Box::new(DepSet::new_atom(deps)))
}

/// Return a package's DEPEND.
///
/// Returns NULL on nonexistence.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_depend(p: *mut Pkg) -> *mut DepSet {
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.depend() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_atom(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.bdepend() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_atom(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.idepend() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_atom(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.pdepend() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_atom(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.rdepend() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_atom(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.license() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_string(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.properties() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_string(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.required_use() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_string(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.restrict() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_string(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.src_uri() {
        None => ptr::null_mut(),
        Some(d) => Box::into_raw(Box::new(DepSet::new_uri(d.clone()))),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    match pkg.long_description() {
        None => ptr::null_mut(),
        Some(s) => CString::new(s).unwrap().into_raw(),
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
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    let mut ptrs: Vec<_> = pkg
        .maintainers()
        .iter()
        .map(|m| {
            let maintainer = Maintainer {
                email: CString::new(m.email()).unwrap().into_raw(),
                name: char_p_or_null!(m.name()),
                description: char_p_or_null!(m.description()),
                maint_type: char_p_or_null!(m.maint_type()),
                proxied: char_p_or_null!(m.proxied()),
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

/// Return a package's upstreams.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_upstreams(
    p: *mut Pkg,
    len: *mut usize,
) -> *mut *mut Upstream {
    let pkg = null_ptr_check!(p.as_ref());
    let (pkg, _) = pkg.as_ebuild().expect("invalid pkg type: {pkg:?}");
    let mut ptrs: Vec<_> = pkg
        .upstreams()
        .iter()
        .map(|m| {
            let upstream = Upstream {
                site: CString::new(m.site()).unwrap().into_raw(),
                name: CString::new(m.name()).unwrap().into_raw(),
            };
            Box::into_raw(Box::new(upstream))
        })
        .collect();
    ptrs.shrink_to_fit();
    unsafe { *len = ptrs.len() };
    let p = ptrs.as_mut_ptr();
    mem::forget(ptrs);
    p
}

/// Free an array of Upstream pointers.
///
/// # Safety
/// The argument must be the value received from pkgcraft_pkg_ebuild_upstreams() or NULL along
/// with the length of the array.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_ebuild_upstreams_free(
    upstreams: *mut *mut Upstream,
    len: usize,
) {
    if !upstreams.is_null() {
        unsafe {
            for ptr in Vec::from_raw_parts(upstreams, len, len).into_iter() {
                drop(Box::from_raw(ptr));
            }
        }
    }
}
