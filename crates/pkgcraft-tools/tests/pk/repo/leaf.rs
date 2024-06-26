use pkgcraft::repo::ebuild::temp::Repo as TempRepo;
use pkgcraft::repo::Repository;
use pkgcraft::test::{cmd, TEST_DATA};
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
    let repo = TempRepo::new("test", None, 0, None).unwrap();
    cmd("pk repo leaf")
        .args([repo.path(), repo.path()])
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn empty_repo() {
    let repo = TEST_DATA.ebuild_repo("empty").unwrap();
    cmd("pk repo leaf")
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn single() {
    let repo = TempRepo::new("test", None, 0, None).unwrap();
    repo.create_raw_pkg("cat/dep-1", &[]).unwrap();
    repo.create_raw_pkg("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    cmd("pk repo leaf")
        .arg(repo.path())
        .assert()
        .stdout("cat/leaf-1\n")
        .stderr("")
        .success();
}

#[test]
fn multiple() {
    let repo = TempRepo::new("test", None, 0, None).unwrap();
    repo.create_raw_pkg("cat/dep-1", &[]).unwrap();
    repo.create_raw_pkg("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    repo.create_raw_pkg("cat/leaf-2", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    cmd("pk repo leaf")
        .arg(repo.path())
        .assert()
        .stdout("cat/leaf-1\ncat/leaf-2\n")
        .stderr("")
        .success();
}

#[test]
fn none() {
    let repo = TempRepo::new("test", None, 0, None).unwrap();
    repo.create_raw_pkg("cat/a-1", &["DEPEND=>=cat/b-1"])
        .unwrap();
    repo.create_raw_pkg("cat/b-1", &["DEPEND=>=cat/a-1"])
        .unwrap();
    cmd("pk repo leaf")
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
