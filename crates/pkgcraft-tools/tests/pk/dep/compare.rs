use itertools::Itertools;
use pkgcraft::test::{cmd, test_data};

use crate::predicates::lines_contain;

#[test]
fn stdin() {
    let data = test_data();
    let exprs = data.dep_toml.compares().map(|(s, _)| s).join("\n");
    cmd("pk dep compare -")
        .write_stdin(exprs)
        .assert()
        .success();

    let exprs = data
        .version_toml
        .compares()
        .map(|(_, (s1, op, s2))| format!("=cat/pkg-{s1} {op} =cat/pkg-{s2}"))
        .join("\n");
    cmd("pk dep compare -")
        .write_stdin(exprs)
        .assert()
        .success();
}

#[test]
fn args() {
    // invalid expression
    cmd("pk dep compare")
        .arg("cat/pkg<cat/pkg")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid comparison format: cat/pkg<cat/pkg"]))
        .failure()
        .code(2);

    // invalid operator
    for op in ["~=", "=", "+="] {
        cmd("pk dep compare")
            .arg(format!("a/b {op} b/c"))
            .assert()
            .stdout("")
            .stderr(lines_contain([format!("invalid operator: a/b {op} b/c")]))
            .failure()
            .code(2);
    }

    // invalid dep
    cmd("pk dep compare")
        .arg("=cat/pkg-1 >= cat/pkg-1")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid dep: cat/pkg-1"]))
        .failure()
        .code(2);

    // false expression
    cmd("pk dep compare")
        .arg("cat/pkg > cat/pkg")
        .assert()
        .failure()
        .code(1);

    // true expression
    cmd("pk dep compare")
        .arg("cat/pkg == cat/pkg")
        .assert()
        .success();
}
