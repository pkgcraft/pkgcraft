use std::ffi::c_char;
use std::slice;

use pkgcraft::eapi::Eapi;

use crate::eapi::eapi_or_default;
use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::repo::Repository;
use crate::types::EbuildTempRepo;

/// Create a temporary ebuild repository.
///
/// Returns NULL on error.
///
/// # Safety
/// The id argument should be a valid, unicode string and the eapi parameter can optionally be
/// NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_new(
    id: *const c_char,
    eapi: *const Eapi,
) -> *mut EbuildTempRepo {
    ffi_catch_panic! {
        let id = try_str_from_ptr!(id);
        let eapi = eapi_or_default!(eapi);
        let repo = unwrap_or_panic!(EbuildTempRepo::new(id, None, 0, Some(eapi)));
        Box::into_raw(Box::new(repo))
    }
}

/// Return a temporary repo's path.
///
/// # Safety
/// The argument must be a non-null EbuildTempRepo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_path(r: *mut EbuildTempRepo) -> *mut c_char {
    let repo = try_ref_from_ptr!(r);
    try_ptr_from_str!(repo.path().as_str())
}

/// Create an ebuild file in the repo.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null EbuildTempRepo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_create_ebuild(
    r: *mut EbuildTempRepo,
    cpv: *const c_char,
    key_vals: *mut *mut c_char,
    len: usize,
) -> *mut c_char {
    ffi_catch_panic! {
        let repo = try_ref_from_ptr!(r);
        let cpv = try_str_from_ptr!(cpv);
        let mut data = vec![];
        for ptr in unsafe { slice::from_raw_parts(key_vals, len) } {
            data.push(try_str_from_ptr!(*ptr));
        }
        let raw_pkg = unwrap_or_panic!(repo.create_raw_pkg(cpv, &data));
        try_ptr_from_str!(raw_pkg.path().as_str())
    }
}

/// Create an ebuild file in the repo from raw data.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null EbuildTempRepo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_create_ebuild_raw(
    r: *mut EbuildTempRepo,
    cpv: *const c_char,
    data: *const c_char,
) -> *mut c_char {
    ffi_catch_panic! {
        let repo = try_ref_from_ptr!(r);
        let cpv = try_str_from_ptr!(cpv);
        let data = try_str_from_ptr!(data);
        let raw_pkg = unwrap_or_panic!(repo.create_raw_pkg_from_str(cpv, data));
        try_ptr_from_str!(raw_pkg.path().as_str())
    }
}

/// Persist a temporary repo to disk, returning its path.
///
/// # Safety
/// The related EbuildTempRepo pointer is invalid on function completion and should not be used.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_persist(
    r: *mut EbuildTempRepo,
    path: *const c_char,
) -> *mut c_char {
    ffi_catch_panic! {
        let repo = unsafe { r.read() };
        let repo_path = if path.is_null() {
            None
        } else {
            Some(try_str_from_ptr!(path))
        };
        let path = unwrap_or_panic!(repo.persist(repo_path));
        try_ptr_from_str!(path.as_str())
    }
}

/// Free a temporary repo.
///
/// Freeing a temporary repo removes the related directory from the filesystem.
///
/// # Safety
/// The argument must be a EbuildTempRepo pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_free(r: *mut EbuildTempRepo) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}
