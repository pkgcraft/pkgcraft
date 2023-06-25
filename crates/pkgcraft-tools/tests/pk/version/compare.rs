use itertools::Itertools;
use pkgcraft::test::{cmd, TEST_DATA};

#[test]
fn args() {
    let exprs = TEST_DATA.version_toml.compares().map(|(s, _)| s).join("\n");
    cmd("pk version compare -")
        .write_stdin(exprs)
        .assert()
        .success();
}
