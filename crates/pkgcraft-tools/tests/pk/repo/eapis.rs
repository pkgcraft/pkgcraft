use pkgcraft::repo::Repository;
use pkgcraft::test::{cmd, TEST_DATA};
use predicates::prelude::*;

#[test]
fn missing_repo_arg() {
    cmd("pk repo eapis")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

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
fn empty_repo() {
    let (_pool, repo) = TEST_DATA.ebuild_repo("empty").unwrap();
    cmd("pk repo eapis")
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn single_repo() {
    let (_pool, repo) = TEST_DATA.ebuild_repo("metadata").unwrap();
    cmd("pk repo eapis")
        .arg(repo.path())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn multiple_repos() {
    let (_pool, repo1) = TEST_DATA.ebuild_repo("metadata").unwrap();
    let (_pool, repo2) = TEST_DATA.ebuild_repo("gentoo").unwrap();
    cmd("pk repo eapis")
        .args([repo1.path(), repo2.path()])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn option_eapi() {
    let (_pool, repo) = TEST_DATA.ebuild_repo("metadata").unwrap();

    // invalid EAPI
    cmd("pk repo eapis --eapi nonexistent")
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);

    // matching packages for EAPI
    cmd("pk repo eapis --eapi 8")
        .arg(repo.path())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // no matching packages for custom EAPI
    cmd("pk repo eapis --eapi pkgcraft")
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
