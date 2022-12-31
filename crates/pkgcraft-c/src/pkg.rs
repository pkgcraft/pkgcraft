use std::cmp::Ordering;
use std::ffi::{c_char, c_int, CString};

use pkgcraft::atom::Atom;
use pkgcraft::pkg::{Package, Pkg};
use pkgcraft::repo::Repo;
use pkgcraft::restrict::{Restrict, Restriction};
use pkgcraft::{eapi, utils::hash};

use crate::macros::*;
use crate::types::AtomVersion;

pub mod ebuild;

#[repr(C)]
pub enum PkgFormat {
    Ebuild,
    Fake,
}

impl From<&Pkg<'_>> for PkgFormat {
    fn from(pkg: &Pkg) -> Self {
        match pkg {
            Pkg::Ebuild(_, _) => Self::Ebuild,
            Pkg::Fake(_, _) => Self::Fake,
        }
    }
}

/// Return a package's format.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_format(p: *mut Pkg) -> PkgFormat {
    let pkg = null_ptr_check!(p.as_ref());
    pkg.into()
}

/// Return a package's atom.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_atom(p: *mut Pkg) -> *const Atom {
    let pkg = null_ptr_check!(p.as_ref());
    pkg.atom()
}

/// Return a package's repo.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_repo(p: *mut Pkg) -> *const Repo {
    let pkg = null_ptr_check!(p.as_ref());
    pkg.repo()
}

/// Return a package's EAPI.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_eapi(p: *mut Pkg) -> *const eapi::Eapi {
    let pkg = null_ptr_check!(p.as_ref());
    pkg.eapi()
}

/// Return a package's version.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_version(p: *mut Pkg) -> *const AtomVersion {
    let pkg = null_ptr_check!(p.as_ref());
    pkg.version()
}

/// Compare two packages returning -1, 0, or 1 if the first package is less than, equal to, or
/// greater than the second package, respectively.
///
/// # Safety
/// The arguments must be non-null Pkg pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_cmp<'a>(p1: *mut Pkg<'a>, p2: *mut Pkg<'a>) -> c_int {
    let pkg1 = null_ptr_check!(p1.as_ref());
    let pkg2 = null_ptr_check!(p2.as_ref());

    match pkg1.cmp(pkg2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the string for a package.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_str(p: *mut Pkg) -> *mut c_char {
    let pkg = null_ptr_check!(p.as_ref());
    CString::new(pkg.to_string()).unwrap().into_raw()
}

/// Return the hash value for a package.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_hash(p: *mut Pkg) -> u64 {
    let pkg = null_ptr_check!(p.as_ref());
    hash(pkg)
}

/// Return the restriction for a package.
///
/// # Safety
/// The argument must be a non-null Pkg pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_restrict(p: *mut Pkg) -> *mut Restrict {
    let pkg = null_ptr_check!(p.as_ref());
    Box::into_raw(Box::new(pkg.into()))
}

/// Determine if a restriction matches a package.
///
/// # Safety
/// The arguments must be valid Restrict and Pkg pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_restrict_matches(pkg: *mut Pkg, r: *mut Restrict) -> bool {
    let pkg = null_ptr_check!(pkg.as_ref());
    let r = null_ptr_check!(r.as_ref());
    r.matches(pkg)
}

/// Free an package.
///
/// # Safety
/// The argument must be a non-null Pkg pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_pkg_free(p: *mut Pkg) {
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)) };
    }
}
