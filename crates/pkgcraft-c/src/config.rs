use std::ffi::{c_char, c_int, CStr};
use std::ptr;

use pkgcraft::config::{Config, RepoSetType};
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::Repo;

use crate::macros::*;

/// Create an empty pkgcraft config.
#[no_mangle]
pub extern "C" fn pkgcraft_config_new() -> *mut Config {
    let config = Config::new("pkgcraft", "");
    Box::into_raw(Box::new(config))
}

/// Add local repo from filesystem path.
///
/// Returns NULL on error.
///
/// # Safety
/// The path argument should be a valid path on the system.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_add_repo_path(
    config: *mut Config,
    id: *const c_char,
    priority: c_int,
    path: *const c_char,
) -> *mut Repo {
    let path = null_ptr_check!(path.as_ref());
    let path = unsafe { unwrap_or_return!(CStr::from_ptr(path).to_str(), ptr::null_mut()) };
    let id = match id.is_null() {
        true => path,
        false => unsafe { unwrap_or_return!(CStr::from_ptr(id).to_str(), ptr::null_mut()) },
    };

    let config = null_ptr_check!(config.as_mut());
    let repo = unwrap_or_return!(config.add_repo_path(id, priority, path), ptr::null_mut());
    Box::into_raw(Box::new(repo))
}

/// Add an external Repo to the config.
///
/// Returns NULL on error.
///
/// # Safety
/// The arguments must be valid Config and Repo pointers.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_add_repo(c: *mut Config, r: *mut Repo) -> *mut Repo {
    let config = null_ptr_check!(c.as_mut());
    let repo = null_ptr_check!(r.as_ref());
    unwrap_or_return!(config.add_repo(repo), ptr::null_mut());
    r
}

/// Load repos from a path to a portage-compatible repos.conf directory or file.
///
/// Returns NULL on error.
///
/// # Safety
/// The path argument should be a valid path on the system.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_load_repos_conf(
    config: *mut Config,
    path: *const c_char,
    len: *mut usize,
) -> *mut *mut Repo {
    let path = null_ptr_check!(path.as_ref());
    let path = unsafe { unwrap_or_return!(CStr::from_ptr(path).to_str(), ptr::null_mut()) };
    let config = null_ptr_check!(config.as_mut());
    let repos = unwrap_or_return!(config.load_repos_conf(path), ptr::null_mut());
    iter_to_array!(repos.into_iter(), len, |r| { Box::into_raw(Box::new(r)) })
}

/// Return the repos for a config.
///
/// # Safety
/// The config argument must be a non-null Config pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_repos(
    config: *mut Config,
    len: *mut usize,
) -> *mut *const Repo {
    // TODO: switch from usize to std::os::raw::c_size_t when it's stable.
    let config = null_ptr_check!(config.as_ref());
    iter_to_array!(config.repos.into_iter(), len, |(_, r)| { r as *const _ })
}

/// Return the RepoSet for a given set type.
///
/// # Safety
/// The config argument must be a non-null Config pointer.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_repos_set(
    config: *mut Config,
    set_type: RepoSetType,
) -> *mut RepoSet {
    let config = null_ptr_check!(config.as_ref());
    Box::into_raw(Box::new(config.repos.set(set_type)))
}

/// Free an array of configured repos.
///
/// # Safety
/// The argument must be the value received from pkgcraft_config_repos() or NULL along with the
/// length of the array.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_repos_free(repos: *mut *mut Repo, len: usize) {
    if !repos.is_null() {
        unsafe { Vec::from_raw_parts(repos, len, len) };
    }
}

/// Free a config.
///
/// # Safety
/// The argument must be a Config pointer or NULL.
#[no_mangle]
pub unsafe extern "C" fn pkgcraft_config_free(config: *mut Config) {
    if !config.is_null() {
        unsafe { drop(Box::from_raw(config)) };
    }
}
