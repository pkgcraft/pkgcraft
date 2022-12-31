#![cfg(test)]

use ctor::ctor;

use crate::shell;

/// Initialize bash for all test executables.
#[ctor]
fn initialize() {
    shell::init(false);
}
