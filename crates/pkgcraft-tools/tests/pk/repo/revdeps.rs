use std::{env, fs};

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn nonexistent_repo() {
    cmd("pk repo revdeps path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("unknown repo: path/to/nonexistent/repo"))
        .failure()
        .code(2);

    cmd("pk repo revdeps nonexistent-repo-alias")
        .assert()
        .stdout("")
        .stderr(contains("unknown repo: nonexistent-repo-alias"))
        .failure()
        .code(2);
}

#[test]
fn ignore() {
    let data = test_data();
    let repo = data.ebuild_repo("bad").unwrap();
    let dir = tempdir().unwrap();
    env::set_current_dir(dir.path()).unwrap();

    // invalid pkgs log errors and cause failure by default
    cmd("pk repo revdeps")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);

    // ignoring invalid pkgs entirely skips them
    for opt in ["-i", "--ignore"] {
        cmd("pk repo revdeps")
            .arg(opt)
            .arg(repo)
            .assert()
            .stdout("")
            .stderr("")
            .success();
    }

    // no directory created since all pkgs are invalid
    assert!(!dir.path().join("revdeps").exists());
}

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    let dir = tempdir().unwrap();
    env::set_current_dir(dir.path()).unwrap();

    cmd("pk repo revdeps")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // no directory created since no pkgs exist
    assert!(!dir.path().join("revdeps").exists());
}

#[test]
fn current_dir_repo() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    repo.create_ebuild("a/b-1", &[]).unwrap();
    repo.create_ebuild("cat/pkg-1", &["DEPEND=a/b"]).unwrap();
    env::set_current_dir(repo.path()).unwrap();

    cmd("pk repo revdeps")
        .assert()
        .stdout("")
        .stderr("")
        .success();

    let data = fs::read_to_string("revdeps/depend/a/b").unwrap();
    assert_eq!(data.trim(), "cat/pkg-1");
}
