use std::cmp::Ordering;
use std::ffi::{CString, c_char, c_int};
use std::ops::Deref;

use pkgcraft::pkg::ebuild::keyword;
use pkgcraft::utils::hash;

use crate::macros::*;
use crate::panic::ffi_catch_panic;
use crate::utils::boxed;

/// Opaque wrapper for pkgcraft::pkg::ebuild::keyword::Keyword.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeywordWrapper(keyword::Keyword);

/// C-compatible wrapper for pkgcraft::pkg::ebuild::keyword::Keyword.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Keyword {
    status: keyword::KeywordStatus,
    arch: *mut c_char,
    keyword: *mut KeywordWrapper,
}

impl From<keyword::Keyword> for Keyword {
    fn from(value: keyword::Keyword) -> Self {
        Keyword {
            status: value.status(),
            arch: try_ptr_from_str!(value.arch().as_ref()),
            keyword: boxed(KeywordWrapper(value)),
        }
    }
}

impl Deref for Keyword {
    type Target = keyword::Keyword;

    fn deref(&self) -> &Self::Target {
        let wrapper = try_ref_from_ptr!(self.keyword);
        &wrapper.0
    }
}

impl Drop for Keyword {
    fn drop(&mut self) {
        unsafe {
            drop(CString::from_raw(self.arch));
            drop(Box::from_raw(self.keyword));
        }
    }
}

/// Parse a string into a package keyword.
///
/// Returns NULL on error.
///
/// # Safety
/// The argument must be a non-null string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_keyword_new(s: *const c_char) -> *mut Keyword {
    ffi_catch_panic! {
        let s = try_str_from_ptr!(s);
        let keyword = unwrap_or_panic!(keyword::Keyword::try_new(s));
        Box::into_raw(Box::new(keyword.into()))
    }
}

/// Compare two package keywords returning -1, 0, or 1 if the first is less than, equal to,
/// or greater than the second, respectively.
///
/// # Safety
/// The arguments must be non-null Keyword pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_keyword_cmp(k1: *mut Keyword, k2: *mut Keyword) -> c_int {
    let k1 = try_ref_from_ptr!(k1);
    let k2 = try_ref_from_ptr!(k2);

    match k1.cmp(k2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Return the hash value for a package keyword.
///
/// # Safety
/// The argument must be a non-null Keyword pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_keyword_hash(k: *mut Keyword) -> u64 {
    let keyword = try_ref_from_ptr!(k);
    hash(keyword.deref())
}

/// Return the string for a package keyword.
///
/// # Safety
/// The argument must be a non-null Keyword pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_keyword_str(k: *mut Keyword) -> *mut c_char {
    let keyword = try_ref_from_ptr!(k);
    try_ptr_from_str!(keyword.to_string())
}

/// Free a package keyword.
///
/// # Safety
/// The argument must be a Keyword pointer or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pkgcraft_keyword_free(k: *mut Keyword) {
    if !k.is_null() {
        unsafe { drop(Box::from_raw(k)) };
    }
}
