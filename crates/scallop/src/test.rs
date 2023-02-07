#![cfg(test)]
use std::env;

/// Initialization for all test executables.
#[ctor::ctor]
fn initialize() {
    // verify running under `cargo nextest`
    env::var("NEXTEST").expect("tests must be run via cargo-nextest");
    // initialize bash for all test executables
    crate::shell::init(false);
}
