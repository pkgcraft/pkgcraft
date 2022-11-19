use std::path::{Path, PathBuf};

use scallop::variables;

// Get the system libdir.
pub(super) fn configure() -> PathBuf {
    PathBuf::from(variables::expand("${ECONF_SOURCE:-.}/configure").unwrap())
}

// Get the system libdir.
pub(super) fn get_libdir(default: Option<&str>) -> Option<String> {
    if let Some(abi) = variables::optional("ABI") {
        if let Some(val) = variables::optional(format!("LIBDIR_{abi}")) {
            return Some(val);
        }
    }
    default.map(|s| s.to_string())
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
