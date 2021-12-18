#![allow(unreachable_pub)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr::null_mut;
use std::str::FromStr;

use crate::atom;

#[repr(C)]
pub struct Atom {
    pub category: *const c_char,
    pub package: *const c_char,
    pub version: *const c_char,
}

/// Parse a string into a package atom.
#[no_mangle]
pub unsafe extern "C" fn str_to_atom(s: *const c_char) -> *mut Atom {
    if s.is_null() {
        return null_mut();
    }

    let cstr = CStr::from_ptr(s);
    // TODO: add error handling
    let atom = atom::Atom::from_str(cstr.to_str().unwrap()).unwrap();

    // parsing should catch errors so no need to check here
    let cat = CString::new(atom.category).unwrap();
    let pkg = CString::new(atom.package).unwrap();
    let ver = match &atom.version {
        Some(v) => CString::new(format!("{}", v)).unwrap(),
        None => CString::new("").unwrap(),
    };

    // create C-compatible struct
    let c_atom = Atom {
        category: cat.into_raw(),
        package: pkg.into_raw(),
        version: ver.into_raw(),
    };

    let boxed = Box::new(c_atom);
    Box::into_raw(boxed)
}

/// Free atom object.
#[no_mangle]
pub unsafe extern "C" fn atom_free(atom: *mut Atom) {
    if atom.is_null() {
        return;
    }

    let a = Box::from_raw(atom);
    let _cat = CString::from_raw(a.category as *mut i8);
    let _pkg = CString::from_raw(a.package as *mut i8);
    let _ver = CString::from_raw(a.version as *mut i8);
}
