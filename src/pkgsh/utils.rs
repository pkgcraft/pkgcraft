use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use itertools::join;

use scallop::variables::{expand, string_value};

// Write the string representing a command to a given handle.
pub(super) fn output_command<W: Write>(mut handle: W, cmd: &Command) {
    write!(handle, "{}", cmd.get_program().to_string_lossy()).unwrap();
    writeln!(handle, " {}", join(cmd.get_args().map(|s| s.to_string_lossy()), " ")).unwrap();
}

// Get the system libdir.
pub(super) fn configure() -> PathBuf {
    PathBuf::from(expand("${ECONF_SOURCE:-.}/configure").unwrap())
}

// Get the system libdir.
pub(super) fn get_libdir(default: Option<&str>) -> Option<String> {
    if let Some(abi) = string_value("ABI") {
        if let Some(val) = string_value(format!("LIBDIR_{abi}")) {
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
