use pkgcraft::test::cmd;
use predicates::prelude::*;

#[test]
fn output() {
    cmd("pkgcruft show checks")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
