use itertools::Itertools;
use pkgcraft::test::{cmd, TEST_DATA};

#[test]
fn args() {
    let exprs = TEST_DATA.dep_toml.compares().map(|(s, _)| s).join("\n");
    cmd("pk dep compare -")
        .write_stdin(exprs)
        .assert()
        .success();

    let exprs = TEST_DATA
        .version_toml
        .compares()
        .map(|(_, (s1, op, s2))| format!("=cat/pkg-{s1} {op} =cat/pkg-{s2}"))
        .join("\n");
    cmd("pk dep compare -")
        .write_stdin(exprs)
        .assert()
        .success();
}
