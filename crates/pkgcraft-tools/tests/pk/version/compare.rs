use itertools::Itertools;
use pkgcraft::test::{cmd, TEST_DATA};

use crate::predicates::lines_contain;

#[test]
fn stdin() {
    let exprs = TEST_DATA.version_toml.compares().map(|(s, _)| s).join("\n");
    cmd("pk version compare -")
        .write_stdin(exprs)
        .assert()
        .success();
}

#[test]
fn args() {
    // invalid expression
    cmd("pk version compare")
        .arg("1<2")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid comparison format: 1<2"]))
        .failure()
        .code(2);

    // invalid operator
    cmd("pk version compare")
        .arg("1 ~= 2")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid operator: ~="]))
        .failure()
        .code(2);

    // false expression
    cmd("pk version compare")
        .arg("1 > 2")
        .assert()
        .failure()
        .code(1);
}
