use std::ffi::{c_char, c_int, CStr};
use std::sync::Arc;
use std::{ptr, slice};

use pkgcraft::repo::fake::Repo as FakeRepo;
use pkgcraft::repo::Repo;

use crate::error::Error;
use crate::macros::*;

/// Create a fake repo from an array of CPV strings.
///
/// Returns NULL on error.
///
/// # Safety
/// The cpvs argument should be valid CPV strings.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_fake_new(
    id: *const c_char,
    priority: c_int,
    cpvs: *mut *mut c_char,
    len: usize,
) -> *mut Repo {
    let c_str = unsafe { CStr::from_ptr(id) };
    let id = unwrap_or_return!(
        c_str
            .to_str()
            .map_err(|e| Error::new(format!("invalid repo id: {c_str:?}: {e}"))),
        ptr::null_mut()
    );
    let mut cpv_strs = vec![];
    unsafe {
        for ptr in slice::from_raw_parts(cpvs, len) {
            let c_str = CStr::from_ptr(*ptr);
            let s = unwrap_or_return!(
                c_str
                    .to_str()
                    .map_err(|e| Error::new(format!("invalid CPV: {c_str:?}: {e}"))),
                ptr::null_mut()
            );
            cpv_strs.push(s);
        }
    }
    let repo = FakeRepo::new(id, priority, cpv_strs);
    Box::into_raw(Box::new(repo.into()))
}

/// Add pkgs to an existing fake repo from an array of CPV strings.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be a non-null Repo pointer and an array of CPV strings.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_fake_extend(
    r: *mut Repo,
    cpvs: *mut *mut c_char,
    len: usize,
) -> *mut Repo {
    let repo = null_ptr_check!(r.as_mut());
    let repo = repo.as_fake_mut().expect("invalid repo type: {repo:?}");
    let repo = unwrap_or_return!(
        Arc::get_mut(repo).ok_or_else(|| Error::new("failed getting mutable repo ref".to_string())),
        ptr::null_mut()
    );

    let mut cpv_strs = vec![];
    unsafe {
        for s in slice::from_raw_parts(cpvs, len) {
            if let Ok(cpv) = CStr::from_ptr(*s).to_str() {
                cpv_strs.push(cpv);
            }
        }
    }

    repo.extend(cpv_strs);
    r
}
