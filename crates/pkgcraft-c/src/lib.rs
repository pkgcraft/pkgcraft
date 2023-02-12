#![warn(unreachable_pub)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_char, CString};

pub mod config;
pub mod dep;
pub mod eapi;
pub mod error;
pub mod free;
pub mod logging;
mod macros;
pub mod opaque;
pub mod parse;
pub mod pkg;
pub mod repo;
pub mod restrict;
pub mod types;
mod utils;

/// Return the library version.
#[no_mangle]
pub extern "C" fn pkgcraft_lib_version() -> *mut c_char {
    let version = env!("CARGO_PKG_VERSION");
    CString::new(version).unwrap().into_raw()
}
