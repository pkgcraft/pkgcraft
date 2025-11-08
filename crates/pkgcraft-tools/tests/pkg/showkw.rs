use pkgcraft::test::test_data;
use predicates::prelude::*;

use crate::cmd;

super::cmd_arg_tests!("pk pkg showkw");

#[test]
fn ignore() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();

    // invalid pkgs log errors and cause failure by default
    cmd("pk pkg showkw")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);

    // ignoring invalid pkgs entirely skips them
    for opt in ["-i", "--ignore"] {
        cmd("pk pkg showkw")
            .arg(opt)
            .arg(repo)
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}
