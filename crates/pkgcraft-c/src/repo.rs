use std::cmp::Ordering;
use std::ffi::{c_char, c_int, CStr, CString};
use std::ptr;

use pkgcraft::pkg::Pkg;
use pkgcraft::repo::{Contains, PkgRepository, Repo, RepoFormat, Repository};
use pkgcraft::restrict::Restrict;
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::types::{RepoPkgIter, RepoRestrictPkgIter};
use crate::utils::str_to_raw;

pub mod ebuild;
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
    finalize: bool,
) -> *mut Repo {
    let path = null_ptr_check!(path.as_ref());
    let path = unsafe { unwrap_or_return!(CStr::from_ptr(path).to_str(), ptr::null_mut()) };
    let id = match id.is_null() {
        true => path,
        false => unsafe { unwrap_or_return!(CStr::from_ptr(id).to_str(), ptr::null_mut()) },
    };

    let repo = unwrap_or_return!(Repo::from_path(id, priority, path, finalize), ptr::null_mut());
    Box::into_raw(Box::new(repo))
}

/// Try to load a certain repo type from a given path.
///
/// Returns NULL on error.
///
/// # Safety
/// The path argument should be a valid path on the system.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_from_format(
    id: *const c_char,
    priority: c_int,
    path: *const c_char,
    format: RepoFormat,
    finalize: bool,
) -> *mut Repo {
    let path = null_ptr_check!(path.as_ref());
    let path = unsafe { unwrap_or_return!(CStr::from_ptr(path).to_str(), ptr::null_mut()) };
    let id = match id.is_null() {
        true => path,
        false => unsafe { unwrap_or_return!(CStr::from_ptr(id).to_str(), ptr::null_mut()) },
    };

    let repo =
        unwrap_or_return!(Repo::from_format(id, priority, path, format, finalize), ptr::null_mut());
    Box::into_raw(Box::new(repo))
}

/// Return a repos's format.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_format(r: *mut Repo) -> RepoFormat {
    let repo = null_ptr_check!(r.as_ref());
    repo.format()
}

/// Return a repo's id.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_id(r: *mut Repo) -> *mut c_char {
    let repo = null_ptr_check!(r.as_ref());
    CString::new(repo.id()).unwrap().into_raw()
}

/// Return a repo's path.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_path(r: *mut Repo) -> *mut c_char {
    let repo = null_ptr_check!(r.as_ref());
    CString::new(repo.path().as_str()).unwrap().into_raw()
}

/// Return a repo's length.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_len(r: *mut Repo) -> usize {
    let repo = null_ptr_check!(r.as_ref());
    repo.len()
}

/// Determine if a repo is empty.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_is_empty(r: *mut Repo) -> bool {
    let r = null_ptr_check!(r.as_ref());
    r.is_empty()
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
    let repo = null_ptr_check!(r.as_ref());
    iter_to_array!(repo.categories().iter(), len, str_to_raw)
}

/// Return a repo's packages for a category.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be a non-null Repo pointer and category.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_packages(
    r: *mut Repo,
    cat: *const c_char,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = null_ptr_check!(r.as_ref());
    let cat = null_ptr_check!(cat.as_ref());
    let cat = unsafe { unwrap_or_return!(CStr::from_ptr(cat).to_str(), ptr::null_mut()) };
    iter_to_array!(repo.packages(cat).iter(), len, str_to_raw)
}

/// Return a repo's versions for a package.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be a non-null Repo pointer, category, and package.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_versions(
    r: *mut Repo,
    cat: *const c_char,
    pkg: *const c_char,
    len: *mut usize,
) -> *mut *mut c_char {
    let repo = null_ptr_check!(r.as_ref());
    let cat = null_ptr_check!(cat.as_ref());
    let pkg = null_ptr_check!(pkg.as_ref());
    let cat = unsafe { unwrap_or_return!(CStr::from_ptr(cat).to_str(), ptr::null_mut()) };
    let pkg = unsafe { unwrap_or_return!(CStr::from_ptr(pkg).to_str(), ptr::null_mut()) };
    iter_to_array!(repo.versions(cat, pkg).iter(), len, str_to_raw)
}

/// Compare two repos returning -1, 0, or 1 if the first repo is less than, equal to, or greater
/// than the second repo, respectively.
///
/// # Safety
/// The arguments must be non-null Repo pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_cmp(r1: *mut Repo, r2: *mut Repo) -> c_int {
    let repo1 = null_ptr_check!(r1.as_ref());
    let repo2 = null_ptr_check!(r2.as_ref());

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
    let repo = null_ptr_check!(r.as_ref());
    hash(repo)
}

/// Determine if a path is in a repo.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_contains_path(r: *mut Repo, path: *const c_char) -> bool {
    let repo = null_ptr_check!(r.as_ref());
    let path = null_ptr_check!(path.as_ref());
    let path = unsafe { unwrap_or_return!(CStr::from_ptr(path).to_str(), false) };
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

/// Return a package iterator for a repo.
///
/// # Safety
/// The argument must be a non-null Repo pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter<'a>(r: *mut Repo) -> *mut RepoPkgIter<'a> {
    let repo = null_ptr_check!(r.as_ref());
    Box::into_raw(Box::new(repo.iter()))
}

/// Return the next package from a package iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null RepoPkgIter pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_next(i: *mut RepoPkgIter) -> *mut Pkg {
    let iter = null_ptr_check!(i.as_mut());
    match iter.next() {
        None => ptr::null_mut(),
        Some(p) => Box::into_raw(Box::new(p)),
    }
}

/// Free a repo iterator.
///
/// # Safety
/// The argument must be a non-null RepoPkgIter pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_iter_free(i: *mut RepoPkgIter) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}

/// Return a restriction package iterator for a repo.
///
/// # Safety
/// The repo argument must be a non-null Repo pointer and the restrict argument must be a non-null
/// Restrict pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_restrict_iter<'a>(
    repo: *mut Repo,
    restrict: *mut Restrict,
) -> *mut RepoRestrictPkgIter<'a> {
    let repo = null_ptr_check!(repo.as_ref());
    let restrict = null_ptr_check!(restrict.as_ref());
    Box::into_raw(Box::new(repo.iter_restrict(restrict.clone())))
}

/// Return the next package from a restriction package iterator.
///
/// Returns NULL when the iterator is empty.
///
/// # Safety
/// The argument must be a non-null RepoRestrictPkgIter pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_restrict_iter_next(i: *mut RepoRestrictPkgIter) -> *mut Pkg {
    let iter = null_ptr_check!(i.as_mut());
    match iter.next() {
        None => ptr::null_mut(),
        Some(p) => Box::into_raw(Box::new(p)),
    }
}

/// Free a repo restriction iterator.
///
/// # Safety
/// The argument must be a non-null RepoRestrictPkgIter pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repo_restrict_iter_free(i: *mut RepoRestrictPkgIter) {
    if !i.is_null() {
        unsafe { drop(Box::from_raw(i)) };
    }
}
