#![allow(unreachable_pub)]

use std::cell::RefCell;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::{fmt, ptr};

use tracing::{error, warn};

use crate::{atom, eapi};

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
    pub use_deps: *const *const c_char,
    // TODO: switch to c_size_t once it's non-experimental
    // https://doc.rust-lang.org/std/os/raw/type.c_size_t.html
    use_deps_len: usize,
    pub repo: *const c_char,
}

/// Parse a string into an atom using a specific EAPI. Pass a null pointer for the eapi argument in
/// order to parse using the latest EAPI with extensions (e.g. support for repo deps).
#[no_mangle]
pub unsafe extern "C" fn str_to_atom(atom: *const c_char, eapi: *const c_char) -> *mut Atom {
    if atom.is_null() {
        let err = PkgcraftError::new("no atom string provided");
        update_last_error(err);
        return ptr::null_mut();
    }

    let atom_str = match CStr::from_ptr(atom).to_str() {
        Ok(s) => s,
        Err(e) => {
            update_last_error(e);
            return ptr::null_mut();
        }
    };

    let eapi = match eapi.is_null() {
        true => &eapi::EAPI_EXTENDED,
        false => match CStr::from_ptr(eapi).to_str() {
            Ok(s) => match eapi::get_eapi(s) {
                Ok(eapi) => eapi,
                Err(e) => {
                    update_last_error(e);
                    return ptr::null_mut();
                }
            },
            Err(e) => {
                update_last_error(e);
                return ptr::null_mut();
            }
        },
    };

    let atom = match atom::parse::dep(atom_str, eapi) {
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

    let mut use_strs = vec![];
    if let Some(use_deps) = atom.use_deps {
        for u in use_deps.iter() {
            use_strs.push(CString::new(u.as_str()).unwrap().into_raw())
        }
    }
    let use_deps_len = use_strs.len();
    // TODO: switch to into_raw_parts() once it's non-experimental
    // https://doc.rust-lang.org/std/vec/struct.Vec.html#method.into_raw_parts
    let use_deps = Box::into_raw(use_strs.into_boxed_slice()).cast();

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
        use_deps,
        use_deps_len,
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
    drop(CString::from_raw(a.category as *mut _));
    drop(CString::from_raw(a.package as *mut _));
    if !a.version.is_null() {
        drop(CString::from_raw(a.version as *mut _));
    }
    if !a.slot.is_null() {
        drop(CString::from_raw(a.slot as *mut _));
    }
    if !a.subslot.is_null() {
        drop(CString::from_raw(a.subslot as *mut _));
    }
    if !a.use_deps.is_null() {
        let use_deps = Vec::from_raw_parts(a.use_deps as *mut _, a.use_deps_len, a.use_deps_len);
        for &u in use_deps.iter() {
            drop(CString::from_raw(u));
        }
    }
    if !a.repo.is_null() {
        drop(CString::from_raw(a.repo as *mut _));
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
