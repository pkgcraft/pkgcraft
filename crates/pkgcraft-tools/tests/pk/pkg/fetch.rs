use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;

#[test]
fn invalid_cwd_target() {
    cmd("pk pkg fetch")
        .assert()
        .stdout("")
        .stderr(contains("invalid ebuild repo"))
        .failure();
}

#[test]
fn nonexistent_path_target() {
    let path = "path/to/nonexistent/repo";
    cmd(format!("pk pkg fetch {path}"))
        .assert()
        .stdout("")
        .stderr(contains(format!("invalid path target: {path}: No such file or directory")))
        .failure();
}

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    cmd("pk pkg fetch")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn invalid_pkgs() {
    let data = test_data();
    let repo = data.ebuild_repo("bad").unwrap();
    cmd("pk pkg fetch")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);
}
