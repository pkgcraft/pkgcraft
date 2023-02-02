#![cfg(test)]

/// Explicitly initialize bash for all test executables.
#[ctor::ctor]
fn initialize() {
    crate::shell::init(false);
}
