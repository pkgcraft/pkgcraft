use itertools::Itertools;
use pkgcraft::test::{cmd, TEST_DATA};
use predicates::prelude::*;

#[test]
fn valid() {
    let intersects = TEST_DATA
        .version_toml
        .intersects
        .iter()
        .flat_map(|e| &e.vals);
    let sorting = TEST_DATA
        .version_toml
        .sorting
        .iter()
        .flat_map(|e| &e.sorted);

    cmd("pk version parse -")
        .write_stdin(intersects.chain(sorting).join("\n"))
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn invalid() {
    cmd("pk version parse 1-r2-3-r4")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();
}
