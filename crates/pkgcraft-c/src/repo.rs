use std::cmp::Ordering;
use std::ffi::{c_char, c_int};
use std::ptr;

use pkgcraft::dep::{Cpv, Version};
use pkgcraft::pkg::Pkg;
use pkgcraft::repo::{PkgRepository, Repo, RepoFormat, Repository};
use pkgcraft::restrict::Restrict;
use pkgcraft::traits::Contains;
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::types::{RepoIter, RepoIterCpv, RepoIterRestrict};
use crate::utils::{boxed, str_to_raw};

pub mod ebuild;
pub mod ebuild_temp;
pub mod fake;
pub mod set;

/// Load a repo from a given path.
///
/// Returns NULL on error.
///
/// # Safety
/// The path argument should be a valid path on the system.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_from_path(
    id: *const c_char,
    priority: c_int,
    path: *const c_char,
) -> *mut Repo {
    ffi_catch_panic! {
        let path = try_str_from_ptr!(path);
        let id = if id.is_null() {
            path
        } else {
            try_str_from_ptr!(id)
        };

        let repo = unwrap_or_panic!(Repo::from_path(id, path, priority));
        Box::into_raw(Box::new(repo))
    }
}

/// Try to load a certain repo type from a given path.
///
/// Returns NULL on error.
///
/// # Safety
/// The path argument should be a valid path on the system.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_from_format(
    format: RepoFormat,
    id: *const c_char,
    priority: c_int,
    path: *const c_char,
) -> *mut Repo {
    ffi_catch_panic! {
        let path = try_str_from_ptr!(path);
        let id = if id.is_null() {
            path
        } else {
            try_str_from_ptr!(id)
        };

        let repo = unwrap_or_panic!(format.load_from_path(id, path, priority));
        Box::into_raw(Box::new(repo))
    }
}

/// Return a repos's format.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_format(r: *mut Repo) -> RepoFormat {
    let repo = try_ref_from_ptr!(r);
    repo.format()
}

/// Return a repo's id.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_id(r: *mut Repo) -> *mut c_char {
    let repo = try_ref_from_ptr!(r);
    try_ptr_from_str!(repo.id())
}

/// Return a repo's path.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_path(r: *mut Repo) -> *mut c_char {
    let repo = try_ref_from_ptr!(r);
    try_ptr_from_str!(repo.path().as_str())
}

/// Return a repo's length.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_len(r: *mut Repo) -> usize {
    let repo = try_ref_from_ptr!(r);
    repo.len()
}

/// Determine if a repo is empty.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_is_empty(r: *mut Repo) -> bool {
    let repo = try_ref_from_ptr!(r);
    repo.is_empty()
}

/// Return a repo's categories.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_categories(
    r: *mut Repo,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = try_ref_from_ptr!(r);
    iter_to_array!(repo.categories().iter(), len, str_to_raw)
}

/// Return a repo's packages for a category.
///
/// # Safety
/// The arguments must be a non-null Repo pointer and category.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_packages(
    r: *mut Repo,
    cat: *const c_char,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = try_ref_from_ptr!(r);
    let cat = try_str_from_ptr!(cat);
    iter_to_array!(repo.packages(cat).iter(), len, str_to_raw)
}

/// Return a repo's versions for a package.
///
/// # Safety
/// The arguments must be a non-null Repo pointer, category, and package.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_versions(
    r: *mut Repo,
    cat: *const c_char,
    pkg: *const c_char,
    len: *mut usize,
) -> *mut *mut Version {
    let repo = try_ref_from_ptr!(r);
    let cat = try_str_from_ptr!(cat);
    let pkg = try_str_from_ptr!(pkg);
    iter_to_array!(repo.versions(cat, pkg).into_iter(), len, boxed)
}

/// Compare two repos returning -1, 0, or 1 if the first repo is less than, equal to, or greater
/// than the second repo, respectively.
///
/// # Safety
/// The arguments must be non-null Repo pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_cmp(r1: *mut Repo, r2: *mut Repo) -> c_int {
    let repo1 = try_ref_from_ptr!(r1);
    let repo2 = try_ref_from_ptr!(r2);

    match repo1.cmp(repo2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the hash value for a repo.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_hash(r: *mut Repo) -> u64 {
    let repo = try_ref_from_ptr!(r);
    hash(repo)
}

/// Determine if a path is in a repo.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_contains_path(r: *mut Repo, path: *const c_char) -> bool {
    let repo = try_ref_from_ptr!(r);
    let path = try_str_from_ptr!(path);
    repo.contains(path)
}

/// Free a repo.
///
/// # Safety
/// The argument must be a Repo pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_free(r: *mut Repo) {
    if !r.is_null() {
        unsafe { drop(Box::from_raw(r)) };
    }
}

/// Return a Cpv iterator for a repo.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_cpv(r: *mut Repo) -> *mut RepoIterCpv {
    let repo = try_ref_from_ptr!(r);
    Box::into_raw(Box::new(repo.iter_cpv()))
}

/// Return the next Cpv from a repo Cpv iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null RepoIterCpv pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_cpv_next(i: *mut RepoIterCpv) -> *mut Cpv {
    let iter = try_mut_from_ptr!(i);
    iter.next().map(boxed).unwrap_or(ptr::null_mut())
}

/// Free a repo Cpv iterator.
///
/// # Safety
/// The argument must be a non-null RepoIterCpv pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_cpv_free(i: *mut RepoIterCpv) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Return a package iterator for a repo.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter(r: *mut Repo) -> *mut RepoIter {
    let repo = try_ref_from_ptr!(r);
    Box::into_raw(Box::new(repo.iter()))
}

/// Return the next package from a package iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null RepoIter pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_next(i: *mut RepoIter) -> *mut Pkg {
    let iter = try_mut_from_ptr!(i);
    // TODO: determine how to differentiate return types for pkg errors and iterator end.
    iter.find_map(|r| r.ok())
        .map(boxed)
        .unwrap_or(ptr::null_mut())
}

/// Free a repo iterator.
///
/// # Safety
/// The argument must be a non-null RepoIter pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_free(i: *mut RepoIter) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Return a restriction iterator for a repo.
///
/// # Safety
/// The repo argument must be a non-null Repo pointer and the restrict argument must be a non-null
/// Restrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_restrict(
    repo: *mut Repo,
    restrict: *mut Restrict,
) -> *mut RepoIterRestrict {
    let repo = try_ref_from_ptr!(repo);
    let restrict = try_ref_from_ptr!(restrict);
    Box::into_raw(Box::new(repo.iter_restrict(restrict.clone())))
}

/// Return the next package from a repo restriction iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null RepoIterRestrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_restrict_next(i: *mut RepoIterRestrict) -> *mut Pkg {
    let iter = try_mut_from_ptr!(i);
    // TODO: determine how to differentiate return types for pkg errors and iterator end.
    iter.find_map(|r| r.ok())
        .map(boxed)
        .unwrap_or(ptr::null_mut())
}

/// Free a repo restriction iterator.
///
/// # Safety
/// The argument must be a non-null RepoIterRestrict pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_restrict_free(i: *mut RepoIterRestrict) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}
