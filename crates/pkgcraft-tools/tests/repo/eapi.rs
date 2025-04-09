use std::env;

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn nonexistent_repo() {
    cmd("pk repo eapi path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("unknown repo: path/to/nonexistent/repo"))
        .failure()
        .code(2);

    cmd("pk repo eapi nonexistent-repo-alias")
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

    // invalid pkgs log errors and cause failure by default
    cmd("pk repo eapi")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);

    // ignoring invalid pkgs entirely skips them
    for opt in ["-i", "--ignore"] {
        cmd("pk repo eapi")
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
    cmd("pk repo eapi")
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
    cmd("pk repo eapi")
        .assert()
        .stdout("")
        .stderr(contains("invalid repo: ."))
        .failure()
        .code(2);

    // repo working directory
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();
    env::set_current_dir(repo).unwrap();
    cmd("pk repo eapi")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn single_repo() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    repo.create_ebuild("cat/pkg-1", &["EAPI=7"]).unwrap();
    repo.create_ebuild("cat/pkg-2", &["EAPI=8"]).unwrap();
    repo.create_ebuild("cat/pkg-3", &["EAPI=8"]).unwrap();

    // all EAPIs
    cmd("pk repo eapi")
        .arg(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            repo
              EAPI 7: 1 pkg
              EAPI 8: 2 pkgs
        "})
        .stderr("")
        .success();

    // invalid, selected EAPI
    cmd("pk repo eapi --eapi nonexistent")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr(contains("unsupported EAPI: nonexistent"))
        .failure()
        .code(2);

    // matching packages for EAPI
    cmd("pk repo eapi --eapi 8")
        .arg(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            cat/pkg-2
            cat/pkg-3
        "})
        .stderr("")
        .success();

    // no matching packages for custom EAPI
    cmd("pk repo eapi --eapi pkgcraft")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn multiple_repos() {
    let mut repo1 = EbuildRepoBuilder::new().name("repo1").build().unwrap();
    repo1.create_ebuild("cat/pkg-1", &["EAPI=7"]).unwrap();
    repo1.create_ebuild("cat/pkg-2", &["EAPI=8"]).unwrap();
    repo1.create_ebuild("cat/pkg-3", &["EAPI=8"]).unwrap();
    let mut repo2 = EbuildRepoBuilder::new().name("repo2").build().unwrap();
    repo2.create_ebuild("cat/pkg-1", &["EAPI=8"]).unwrap();

    cmd("pk repo eapi")
        .args([&repo1, &repo2])
        .assert()
        .stdout(indoc::indoc! {"
            repo1
              EAPI 7: 1 pkg
              EAPI 8: 2 pkgs
            repo2
              EAPI 8: 1 pkg
        "})
        .stderr("")
        .success();
}
