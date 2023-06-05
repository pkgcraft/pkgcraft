use std::os::fd::RawFd;
use std::path::{Path, PathBuf};

use nix::unistd::{close, dup2};
use scallop::variables;

/// Redirect stdout and stderr to a given raw file descriptor.
pub(super) fn redirect_output(fd: RawFd) -> scallop::Result<()> {
    dup2(fd, 1)?;
    dup2(fd, 2)?;
    close(fd)?;
    Ok(())
}

// Get the path to a package's configure script.
pub(super) fn configure() -> PathBuf {
    PathBuf::from(variables::expand("${ECONF_SOURCE:-.}/configure").unwrap())
}

// Get the system libdir.
pub(super) fn get_libdir(default: Option<&str>) -> Option<String> {
    variables::optional("ABI")
        .and_then(|abi| variables::optional(format!("LIBDIR_{abi}")))
        .or_else(|| default.map(|s| s.to_string()))
}

// Check if a compatible makefile exists in the current working directory.
pub(super) fn makefile_exists() -> bool {
    ["Makefile", "GNUmakefile", "makefile"]
        .iter()
        .any(|f| Path::new(f).is_file())
}
