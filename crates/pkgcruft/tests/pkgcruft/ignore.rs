use std::env;

use pkgcraft::test::{cmd, test_data};
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    cmd("pkgcruft ignore")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn current_dir_targets() {
    // empty dir
    let tmpdir = tempdir().unwrap();
    let path = tmpdir.path().to_str().unwrap();
    env::set_current_dir(path).unwrap();
    cmd("pkgcruft ignore")
        .assert()
        .stdout("")
        .stderr(contains("invalid ebuild repo: ."))
        .failure()
        .code(2);
}
