use std::ffi::c_char;
use std::sync::Arc;

use pkgcraft::eapi::Eapi;
use pkgcraft::repo::ebuild::Repo as EbuildRepo;
use pkgcraft::repo::Repo;

use crate::macros::*;
use crate::utils::str_to_raw;

/// Return an ebuild repo's metadata arches.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_arches(
    r: *mut Repo,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = null_ptr_check!(r.as_ref());
    let repo = repo.as_ebuild().expect("invalid repo type: {repo:?}");
    iter_to_array!(repo.metadata().arches().iter(), len, str_to_raw)
}

/// Return an ebuild repo's metadata categories.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_categories(
    r: *mut Repo,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = null_ptr_check!(r.as_ref());
    let repo = repo.as_ebuild().expect("invalid repo type: {repo:?}");
    iter_to_array!(repo.metadata().categories().iter(), len, str_to_raw)
}

/// Return an ebuild repo's EAPI.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_eapi(r: *mut Repo) -> *const Eapi {
    let repo = null_ptr_check!(r.as_ref());
    let repo = repo.as_ebuild().expect("invalid repo type: {repo:?}");
    repo.eapi()
}

/// Return an ebuild repo's masters.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_masters(
    r: *mut Repo,
    len: *mut usize,
) -> *mut *mut Repo {
    let repo = null_ptr_check!(r.as_ref());
    let repo = repo.as_ebuild().expect("invalid repo type: {repo:?}");
    iter_to_array!(repo.masters().iter(), len, |r: &Arc<EbuildRepo>| {
        Box::into_raw(Box::new(Repo::Ebuild(r.clone())))
    })
}
