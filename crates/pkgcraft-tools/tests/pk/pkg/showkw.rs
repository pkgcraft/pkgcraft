use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;

super::cmd_arg_tests!("pk pkg showkw");

#[test]
fn invalid_pkgs() {
    let data = test_data();
    let repo = data.ebuild_repo("bad").unwrap();
    cmd("pk pkg showkw")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);
}
