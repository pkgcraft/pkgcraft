#![cfg(test)]
use std::env;

/// Initialization for all test executables.
#[ctor::ctor]
fn initialize() {
    // verify running under `cargo nextest` ignoring benchmark runs
    if !env::args().any(|x| x == "--bench") {
        env::var("NEXTEST").expect("tests must be run via cargo-nextest");
    }

    // initialize bash
    crate::shell::init(crate::shell::Env::default());
}
