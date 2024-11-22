use pkgcraft::config::Config;
use pkgcraft::repo::Repository;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;

#[test]
fn missing_repo_arg() {
    cmd("pk repo leaf")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

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
    let mut config = Config::default();
    let temp = config.temp_repo("test", 0, None).unwrap();
    cmd("pk repo leaf")
        .args([temp.path(), temp.path()])
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    cmd("pk repo leaf")
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn single() {
    let mut config = Config::default();
    let mut temp = config.temp_repo("test", 0, None).unwrap();
    temp.create_ebuild("cat/dep-1", &[]).unwrap();
    temp.create_ebuild("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    cmd("pk repo leaf")
        .arg(temp.path())
        .assert()
        .stdout("cat/leaf-1\n")
        .stderr("")
        .success();
}

#[test]
fn multiple() {
    let mut config = Config::default();
    let mut temp = config.temp_repo("test", 0, None).unwrap();
    temp.create_ebuild("cat/dep-1", &[]).unwrap();
    temp.create_ebuild("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    temp.create_ebuild("cat/leaf-2", &["DEPEND=>=cat/dep-1"])
        .unwrap();

    let output = cmd("pk repo leaf").arg(temp.path()).output().unwrap();
    let sorted: Vec<_> = std::str::from_utf8(&output.stdout)
        .unwrap()
        .split_whitespace()
        .collect();
    assert_eq!(&sorted, &["cat/leaf-1", "cat/leaf-2"]);
}

#[test]
fn none() {
    let mut config = Config::default();
    let mut temp = config.temp_repo("test", 0, None).unwrap();
    temp.create_ebuild("cat/a-1", &["DEPEND=>=cat/b-1"])
        .unwrap();
    temp.create_ebuild("cat/b-1", &["DEPEND=>=cat/a-1"])
        .unwrap();
    cmd("pk repo leaf")
        .arg(temp.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
