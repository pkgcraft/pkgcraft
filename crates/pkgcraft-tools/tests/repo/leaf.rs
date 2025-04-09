use std::env;

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn nonexistent_repo() {
    cmd("pk repo leaf path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("unknown repo: path/to/nonexistent/repo"))
        .failure()
        .code(2);

    cmd("pk repo leaf nonexistent-repo-alias")
        .assert()
        .stdout("")
        .stderr(contains("unknown repo: nonexistent-repo-alias"))
        .failure()
        .code(2);
}

#[test]
fn multiple_repos_not_supported() {
    cmd("pk repo leaf")
        .args(["repo1", "repo2"])
        .assert()
        .stdout("")
        .stderr(contains("unexpected argument 'repo2' found"))
        .failure()
        .code(2);
}

#[test]
fn ignore() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();

    // invalid pkgs log errors and cause failure by default
    cmd("pk repo leaf")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);

    // ignoring invalid pkgs entirely skips them
    for opt in ["-i", "--ignore"] {
        cmd("pk repo leaf")
            .arg(opt)
            .arg(repo)
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
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
    // non-repo working directory
    let dir = tempdir().unwrap();
    env::set_current_dir(dir.path()).unwrap();
    cmd("pk repo leaf")
        .assert()
        .stdout("")
        .stderr(contains("invalid repo: ."))
        .failure()
        .code(2);

    // repo working directory
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
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    repo.create_ebuild("cat/dep-1", &[]).unwrap();
    repo.create_ebuild("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();

    cmd("pk repo leaf")
        .arg(&repo)
        .assert()
        .stdout("cat/leaf-1\n")
        .stderr("")
        .success();
}

#[test]
fn multiple() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    repo.create_ebuild("cat/dep-1", &[]).unwrap();
    repo.create_ebuild("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    repo.create_ebuild("cat/leaf-2", &["DEPEND=>=cat/dep-1"])
        .unwrap();

    cmd("pk repo leaf")
        .arg(&repo)
        .assert()
        .stdout("cat/leaf-1\ncat/leaf-2\n")
        .stderr("")
        .success();
}

#[test]
fn none() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    repo.create_ebuild("cat/a-1", &["DEPEND=>=cat/b-1"])
        .unwrap();
    repo.create_ebuild("cat/b-1", &["DEPEND=>=cat/a-1"])
        .unwrap();

    cmd("pk repo leaf")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
