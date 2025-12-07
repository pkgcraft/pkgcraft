#![cfg(test)]

/// Initialization for all test executables.
#[ctor::ctor]
fn initialize() {
    // initialize bash
    crate::shell::init(crate::shell::Env::default());
}
