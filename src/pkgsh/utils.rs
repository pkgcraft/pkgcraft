use std::path::Path;

// Check if a compatible makefile exists in the current working directory.
pub(super) fn makefile_exists() -> bool {
    for f in ["Makefile", "GNUmakefile", "makefile"] {
        if Path::new(f).exists() {
            return true;
        }
    }
    false
}
