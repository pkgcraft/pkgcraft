use std::ffi::c_char;
use std::sync::Arc;

use pkgcraft::eapi::Eapi;
use pkgcraft::repo::ebuild::Repo as EbuildRepo;
use pkgcraft::repo::Repo;

use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::utils::str_to_raw;

/// Convert a given pointer into an ebuild repo reference.
macro_rules! try_repo_from_ptr {
    ( $var:expr ) => {{
        let repo = $crate::macros::try_ref_from_ptr!($var);
        match repo.as_ebuild() {
            Some(r) => r,
            None => panic!("invalid repo type: {repo:?}"),
        }
    }};
}

/// Return an ebuild repo's metadata arches.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_metadata_arches(
    r: *mut Repo,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = try_repo_from_ptr!(r);
    iter_to_array!(repo.metadata().arches().iter(), len, str_to_raw)
}

/// Return an ebuild repo's metadata categories.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_metadata_categories(
    r: *mut Repo,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = try_repo_from_ptr!(r);
    iter_to_array!(repo.metadata().categories().iter(), len, str_to_raw)
}

/// Return an ebuild repo's EAPI.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_eapi(r: *mut Repo) -> *const Eapi {
    let repo = try_repo_from_ptr!(r);
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
    let repo = try_repo_from_ptr!(r);
    iter_to_array!(repo.masters(), len, |r: Arc<EbuildRepo>| {
        Box::into_raw(Box::new(Repo::Ebuild(r)))
    })
}

/// Regenerate an ebuild repo's package metadata cache.
///
/// Returns false on error, otherwise true.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_pkg_metadata_regen(
    r: *mut Repo,
    jobs: usize,
    force: bool,
) -> bool {
    ffi_catch_panic! {
        let repo = try_repo_from_ptr!(r);
        let errors = unwrap_or_panic!(repo.pkg_metadata_regen(jobs, force));
        errors > 0
    }
}
