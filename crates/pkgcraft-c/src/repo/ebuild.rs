use std::sync::Arc;

use pkgcraft::eapi::Eapi;
use pkgcraft::repo::{ebuild::Repo as EbuildRepo, Repo};

use crate::macros::*;

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
