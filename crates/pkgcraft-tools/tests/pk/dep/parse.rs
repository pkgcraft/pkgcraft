use itertools::Itertools;
use pkgcraft::test::{cmd, TEST_DATA};
use predicates::prelude::*;

#[test]
fn valid() {
    let deps = TEST_DATA.dep_toml.valid.iter().map(|e| &e.dep).join("\n");
    cmd("pk dep parse -")
        .write_stdin(deps)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn invalid() {
    let deps = TEST_DATA.dep_toml.invalid.iter().join("\n");
    cmd("pk dep parse -")
        .write_stdin(deps)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();
}

#[test]
fn eapi() {
    // use deps in EAPI >= 2
    cmd("pk dep parse --eapi 0 cat/pkg[use]")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();

    cmd("pk dep parse --eapi 2 cat/pkg[use]")
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
