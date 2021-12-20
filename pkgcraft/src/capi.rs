#![allow(unreachable_pub)]

use std::cell::RefCell;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::{fmt, ptr, str::FromStr};

use tracing::{error, warn};

use crate::atom::Atom as PkgAtom;

#[derive(Debug)]
struct PkgcraftError {
    pub message: String,
}

impl PkgcraftError {
    fn new(msg: &str) -> PkgcraftError {
        PkgcraftError {
            message: msg.to_string(),
        }
    }
}

impl fmt::Display for PkgcraftError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for PkgcraftError {}

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
        let err = PkgcraftError::new("no atom string provided");
        update_last_error(err);
        return ptr::null_mut();
    }

    let cstr = CStr::from_ptr(s);
    let atom_str = match cstr.to_str() {
        Ok(s) => s,
        Err(e) => {
            update_last_error(e);
            return ptr::null_mut();
        }
    };

    let atom = match PkgAtom::from_str(atom_str) {
        Ok(a) => a,
        Err(e) => {
            update_last_error(e);
            return ptr::null_mut();
        }
    };

    // parsing should catch errors so no need to check here
    let category = CString::new(atom.category).unwrap().into_raw();
    let package = CString::new(atom.package).unwrap().into_raw();
    let version = match atom.version {
        Some(s) => CString::new(format!("{}", s)).unwrap().into_raw(),
        None => ptr::null(),
    };
    let slot = match atom.slot {
        Some(s) => CString::new(s).unwrap().into_raw(),
        None => ptr::null(),
    };
    let subslot = match atom.subslot {
        Some(s) => CString::new(s).unwrap().into_raw(),
        None => ptr::null(),
    };
    let repo = match atom.repo {
        Some(s) => CString::new(s).unwrap().into_raw(),
        None => ptr::null(),
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

thread_local! {
    static LAST_ERROR: RefCell<Option<Box<dyn Error>>> = RefCell::new(None);
}

/// Update the most recent error, clearing the previous value.
pub fn update_last_error<E: Error + 'static>(err: E) {
    error!("Setting LAST_ERROR: {}", err);

    {
        // Print a pseudo-backtrace for this error, following back each error's
        // source until we reach the root error.
        let mut source = err.source();
        while let Some(parent_err) = source {
            warn!("Caused by: {}", parent_err);
            source = parent_err.source();
        }
    }

    LAST_ERROR.with(|prev| {
        *prev.borrow_mut() = Some(Box::new(err));
    });
}

/// Get the most recent error message as a UTF-8 string, if none exists a null pointer is returned.
///
/// The caller is expected to free memory used by the string after they're finished using it.
#[no_mangle]
pub unsafe extern "C" fn last_error_message() -> *mut c_char {
    // Retrieve the most recent error, clearing it in the process.
    let last_error: Option<Box<dyn Error>> = LAST_ERROR.with(|prev| prev.borrow_mut().take());
    match last_error {
        Some(e) => CString::new(e.to_string()).unwrap().into_raw(),
        None => return ptr::null_mut(),
    }
}
