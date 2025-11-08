use itertools::Itertools;
use pkgcraft::test::test_data;

use crate::cmd;
use crate::predicates::lines_contain;

#[test]
fn stdin() {
    let data = test_data();
    let exprs = data
        .version_toml
        .compares()
        .map(|(_, (s1, op, s2))| format!("cat/pkg-{s1} {op} cat/pkg-{s2}"))
        .join("\n");
    cmd("pk cpv compare -")
        .write_stdin(exprs)
        .assert()
        .success();
}

#[test]
fn args() {
    // invalid expression
    cmd("pk cpv compare")
        .arg("cat/pkg-1<cat/pkg-2")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid comparison format: cat/pkg-1<cat/pkg-2"]))
        .failure()
        .code(2);

    // invalid operator
    for op in ["~=", "=", "+="] {
        cmd("pk cpv compare")
            .arg(format!("cat/pkg-1 {op} cat/pkg-2"))
            .assert()
            .stdout("")
            .stderr(lines_contain([format!("invalid operator: cat/pkg-1 {op} cat/pkg-2")]))
            .failure()
            .code(2);
    }

    // invalid cpv
    cmd("pk cpv compare")
        .arg("=cat/pkg-1 >= cat/pkg-1")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid cpv: =cat/pkg-1"]))
        .failure()
        .code(2);

    // false expression
    cmd("pk cpv compare")
        .arg("cat/pkg-1 > cat/pkg-2")
        .assert()
        .failure()
        .code(1);

    // true expression
    cmd("pk cpv compare")
        .arg("cat/pkg-1 == cat/pkg-1-r0")
        .assert()
        .success();
}
