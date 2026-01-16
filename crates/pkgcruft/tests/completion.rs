use std::fs;

use camino_tempfile::tempdir;
use predicates::prelude::*;

use crate::cmd;

#[test]
fn no_target() {
    cmd("pkgcruft completion")
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
        cmd("pkgcruft completion")
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
    cmd("pkgcruft completion unknown")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);

    // valid
    cmd("pkgcruft completion zsh")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
