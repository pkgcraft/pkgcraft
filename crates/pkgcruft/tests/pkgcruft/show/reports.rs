use pkgcraft::test::cmd;
use predicates::prelude::*;

#[test]
fn output() {
    cmd("pkgcruft show reports")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn color() {
    for opt in ["-c", "--color"] {
        cmd("pkgcruft show reports")
            .arg(opt)
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}
