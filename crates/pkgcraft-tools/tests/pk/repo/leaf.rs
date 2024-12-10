use std::env;

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;

#[test]
fn nonexistent_repo() {
    cmd("pk repo leaf path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn multiple_repos_not_supported() {
    let temp = EbuildRepoBuilder::new().build().unwrap();
    let path = temp.path();

    cmd("pk repo leaf")
        .args([path, path])
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

    cmd("pk repo leaf")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);
}

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();

    cmd("pk repo leaf")
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

    cmd("pk repo leaf")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn single() {
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/dep-1", &[]).unwrap();
    temp.create_ebuild("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    let path = temp.path();

    cmd("pk repo leaf")
        .arg(path)
        .assert()
        .stdout("cat/leaf-1\n")
        .stderr("")
        .success();
}

#[test]
fn multiple() {
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/dep-1", &[]).unwrap();
    temp.create_ebuild("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    temp.create_ebuild("cat/leaf-2", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    let path = temp.path();

    cmd("pk repo leaf")
        .arg(path)
        .assert()
        .stdout("cat/leaf-1\ncat/leaf-2\n")
        .stderr("")
        .success();
}

#[test]
fn none() {
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/a-1", &["DEPEND=>=cat/b-1"])
        .unwrap();
    temp.create_ebuild("cat/b-1", &["DEPEND=>=cat/a-1"])
        .unwrap();
    let path = temp.path();

    cmd("pk repo leaf")
        .arg(path)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
