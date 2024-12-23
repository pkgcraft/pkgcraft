use std::env;

use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;

#[test]
fn nonexistent_repo() {
    cmd("pk repo eapis path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn invalid_pkgs() {
    let data = test_data();
    let repo = data.ebuild_repo("bad").unwrap();
    cmd("pk repo eapis")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);
}

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    cmd("pk repo eapis")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn default_current_directory() {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();
    env::set_current_dir(repo).unwrap();
    cmd("pk repo eapis")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn single_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();
    cmd("pk repo eapis")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn multiple_repos() {
    let data = test_data();
    let repo1 = data.ebuild_repo("metadata").unwrap();
    let repo2 = data.ebuild_repo("gentoo").unwrap();
    cmd("pk repo eapis")
        .args([repo1.path(), repo2.path()])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn option_eapi() {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();

    // invalid EAPI
    cmd("pk repo eapis --eapi nonexistent")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);

    // matching packages for EAPI
    cmd("pk repo eapis --eapi 8")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // no matching packages for custom EAPI
    cmd("pk repo eapis --eapi pkgcraft")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
