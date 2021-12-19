#![allow(unreachable_pub)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr::{null, null_mut};
use std::str::FromStr;

use crate::atom::Atom as PkgAtom;

#[repr(C)]
pub struct Atom {
    pub category: *const c_char,
    pub package: *const c_char,
    pub version: *const c_char,
    pub slot: *const c_char,
    pub subslot: *const c_char,
    pub repo: *const c_char,
}

/// Parse a string into a package atom.
#[no_mangle]
pub unsafe extern "C" fn str_to_atom(s: *const c_char) -> *mut Atom {
    if s.is_null() {
        return null_mut();
    }

    let cstr = CStr::from_ptr(s);
    // TODO: add error handling
    let atom = PkgAtom::from_str(cstr.to_str().unwrap()).unwrap();

    // parsing should catch errors so no need to check here
    let category = CString::new(atom.category).unwrap().into_raw();
    let package = CString::new(atom.package).unwrap().into_raw();
    let version = match atom.version {
        Some(s) => CString::new(format!("{}", s)).unwrap().into_raw(),
        None => null(),
    };
    let slot = match atom.slot {
        Some(s) => CString::new(s).unwrap().into_raw(),
        None => null(),
    };
    let subslot = match atom.subslot {
        Some(s) => CString::new(s).unwrap().into_raw(),
        None => null(),
    };
    let repo = match atom.repo {
        Some(s) => CString::new(s).unwrap().into_raw(),
        None => null(),
    };

    // create C-compatible struct
    let c_atom = Atom {
        category,
        package,
        version,
        slot,
        subslot,
        repo,
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
    if !(*atom).version.is_null() {
        let _ver = CString::from_raw(a.version as *mut i8);
    }
    if !(*atom).slot.is_null() {
        let _slot = CString::from_raw(a.slot as *mut i8);
    }
    if !(*atom).subslot.is_null() {
        let _subslot = CString::from_raw(a.subslot as *mut i8);
    }
    if !(*atom).repo.is_null() {
        let _repo = CString::from_raw(a.repo as *mut i8);
    }
}
