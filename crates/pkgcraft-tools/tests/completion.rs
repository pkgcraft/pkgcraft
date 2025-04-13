use std::fs;

use pkgcraft::test::cmd;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn no_target() {
    cmd("pk completion")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn dir() {
    let dir = tempdir().unwrap();
    for opt in ["-d", "--dir"] {
        cmd("pk completion")
            .arg(opt)
            .arg(dir.path())
            .assert()
            .stdout("")
            .stderr("")
            .success();
        assert!(fs::read_dir(dir.path()).unwrap().next().is_some());
    }
}

#[test]
fn target() {
    // invalid
    cmd("pk completion unknown")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);

    // valid
    cmd("pk completion zsh")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
