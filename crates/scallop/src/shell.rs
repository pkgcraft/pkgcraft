use std::ffi::CStr;

use once_cell::sync::Lazy;

use crate::bash;

// conditionally export forked bash functionality
#[cfg(not(feature = "plugin"))]
pub use crate::scallop::shell::*;

/// Returns true if currently operating in a subshell, false otherwise.
pub fn in_subshell() -> bool {
    subshell_level() > 0
}

/// Returns the count of nested subshells (also available via $BASH_SUBSHELL).
pub fn subshell_level() -> i32 {
    unsafe { bash::SUBSHELL_LEVEL }
}

/// Returns true if currently operating in restricted mode.
pub fn is_restricted() -> bool {
    unsafe { bash::RESTRICTED != 0 }
}

/// Returns true if shell started in restricted mode.
pub fn is_restricted_shell() -> bool {
    unsafe { bash::RESTRICTED_SHELL != 0 }
}

/// Version string related to the bundled bash release.
pub static BASH_VERSION: Lazy<String> = Lazy::new(|| unsafe {
    let version = CStr::from_ptr(bash::DIST_VERSION).to_str().unwrap();
    format!("{version}.{}", bash::PATCH_LEVEL)
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_version() {
        // TODO: add simple comparison check with version-compare if upstream merges set opts patch
        assert!(!BASH_VERSION.is_empty());
    }
}
