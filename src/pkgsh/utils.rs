use std::path::{Path, PathBuf};

use scallop::variables::{expand, string_value};

// Get the system libdir.
pub(super) fn configure() -> PathBuf {
    PathBuf::from(expand("${ECONF_SOURCE:-.}/configure").unwrap())
}

// Get the system libdir.
pub(super) fn get_libdir() -> String {
    let mut libdir = String::from("lib");
    if let Some(abi) = string_value("ABI") {
        if let Some(val) = string_value(format!("LIBDIR_{}", abi)) {
            libdir = val;
        }
    }
    libdir
}

// Check if a compatible makefile exists in the current working directory.
pub(super) fn makefile_exists() -> bool {
    for f in ["Makefile", "GNUmakefile", "makefile"] {
        if Path::new(f).exists() {
            return true;
        }
    }
    false
}
