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
    let env: [(&str, &str); 0] = [];
    crate::shell::init(env);
}
