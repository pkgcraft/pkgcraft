use std::ffi::c_char;
use std::sync::Arc;

use pkgcraft::config::Config;
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
        match repo {
            Repo::Ebuild(r) => r.as_ref(),
            Repo::Configured(r) => r.as_ref().into(),
            _ => panic!("invalid repo type: {repo:?}"),
        }
    }};
}

/// Return a configured repo using the given config.
///
/// # Safety
/// The arguments must be valid Repo and Config pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_configure(r: *mut Repo, c: *mut Config) -> *mut Repo {
    let repo = try_repo_from_ptr!(r);
    let config = try_ref_from_ptr!(c);
    Box::into_raw(Box::new(repo.configure(config).into()))
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

/// Return an ebuild repo's metadata licenses.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_metadata_licenses(
    r: *mut Repo,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = try_repo_from_ptr!(r);
    iter_to_array!(repo.metadata().licenses().iter(), len, str_to_raw)
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

/// Return an ebuild repo's inherited licenses.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_licenses(
    r: *mut Repo,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = try_repo_from_ptr!(r);
    iter_to_array!(repo.licenses().iter(), len, str_to_raw)
}

/// Regenerate an ebuild repo's package metadata cache.
///
/// Returns false on error, otherwise true.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_ebuild_metadata_regen(
    r: *mut Repo,
    jobs: usize,
    force: bool,
    path: *mut c_char,
) -> bool {
    ffi_catch_panic! {
        let repo = try_repo_from_ptr!(r);
        let path = try_opt_str_from_ptr!(path).unwrap_or_default();
        let runner = repo.metadata_regen().jobs(jobs).force(force).cache_path(path);
        unwrap_or_panic!(runner.run());
        true
    }
}
