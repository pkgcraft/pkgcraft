use std::ffi::{c_char, c_int};
use std::slice;

use pkgcraft::eapi::Eapi;
use pkgcraft::repo::ebuild::{EbuildRepoBuilder, EbuildTempRepo};

use crate::eapi::eapi_or_default;
use crate::macros::*;
use crate::panic::ffi_catch_panic;

/// Create a temporary ebuild repository.
///
/// Returns NULL on error.
///
/// # Safety
/// The name argument should be a valid, unicode string and the eapi parameter can optionally be
/// NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_new(
    name: *const c_char,
    eapi: *const Eapi,
    priority: c_int,
) -> *mut EbuildTempRepo {
    ffi_catch_panic! {
        let name = try_str_from_ptr!(name);
        let eapi = eapi_or_default!(eapi);
        let temp = EbuildRepoBuilder::new().name(name).priority(priority).eapi(eapi).build();
        Box::into_raw(Box::new(unwrap_or_panic!(temp)))
    }
}

/// Return a temporary repo's path.
///
/// # Safety
/// The argument must be a non-null EbuildTempRepo pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_path(
    r: *mut EbuildTempRepo,
) -> *mut c_char {
    let temp = try_ref_from_ptr!(r);
    try_ptr_from_str!(temp.path().as_str())
}

/// Create an ebuild package in the repo.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null EbuildTempRepo pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_create_ebuild(
    r: *mut EbuildTempRepo,
    cpv: *const c_char,
    key_vals: *mut *mut c_char,
    len: usize,
) -> *mut c_char {
    ffi_catch_panic! {
        let temp = try_mut_from_ptr!(r);
        let cpv = try_str_from_ptr!(cpv);
        let mut data = vec![];
        for ptr in unsafe { slice::from_raw_parts(key_vals, len) } {
            data.push(try_str_from_ptr!(*ptr));
        }
        let path = unwrap_or_panic!(temp.create_ebuild(cpv, &data));
        try_ptr_from_str!(path.as_str())
    }
}

/// Create an ebuild package in the repo from raw data.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null EbuildTempRepo pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_create_ebuild_from_str(
    r: *mut EbuildTempRepo,
    cpv: *const c_char,
    data: *const c_char,
) -> *mut c_char {
    ffi_catch_panic! {
        let temp = try_mut_from_ptr!(r);
        let cpv = try_str_from_ptr!(cpv);
        let data = try_str_from_ptr!(data);
        let path = unwrap_or_panic!(temp.create_ebuild_from_str(cpv, data));
        try_ptr_from_str!(path.as_str())
    }
}

/// Persist a temporary repo to disk, returning its path.
///
/// # Safety
/// The related EbuildTempRepo pointer is invalid on function completion and should not be used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_persist(
    r: *mut EbuildTempRepo,
) -> *mut c_char {
    ffi_catch_panic! {
        let temp = unsafe { r.read() };
        let path = unwrap_or_panic!(temp.persist());
        try_ptr_from_str!(path.as_str())
    }
}

/// Free a temporary repo.
///
/// Freeing a temporary repo removes the related directory from the filesystem.
///
/// # Safety
/// The argument must be a EbuildTempRepo pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_temp_free(r: *mut EbuildTempRepo) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}
