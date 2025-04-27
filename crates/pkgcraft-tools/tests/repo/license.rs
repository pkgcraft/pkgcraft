use std::{env, fs};

use pkgcraft::repo::Repository;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn nonexistent_repo() {
    cmd("pk repo license path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("nonexistent repo: path/to/nonexistent/repo"))
        .failure()
        .code(2);

    cmd("pk repo license nonexistent-repo-alias")
        .assert()
        .stdout("")
        .stderr(contains("nonexistent repo: nonexistent-repo-alias"))
        .failure()
        .code(2);
}

#[test]
fn ignore() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();

    // invalid pkgs log errors and cause failure by default
    cmd("pk repo license")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);

    // ignoring invalid pkgs entirely skips them
    for opt in ["-i", "--ignore"] {
        cmd("pk repo license")
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
    cmd("pk repo license")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn current_directory() {
    // invalid repo
    let dir = tempdir().unwrap();
    env::set_current_dir(dir.path()).unwrap();
    cmd("pk repo license")
        .assert()
        .stdout("")
        .stderr(contains("invalid repo: ."))
        .failure()
        .code(2);

    let data = test_data();
    let repo_path = data.repo("metadata").unwrap().path();

    // repo directory
    env::set_current_dir(repo_path).unwrap();
    cmd("pk repo license")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // category directory
    env::set_current_dir(repo_path.join("slot")).unwrap();
    cmd("pk repo license")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // package directory
    env::set_current_dir(repo_path.join("slot/slot")).unwrap();
    cmd("pk repo license")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn single_repo() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    let dir = repo.path().join("licenses");
    fs::create_dir(&dir).unwrap();
    let data = indoc::indoc! {r#"
        # stub license
    "#};
    fs::write(dir.join("l1"), data).unwrap();
    let data = indoc::indoc! {r#"
        # stub license
    "#};
    fs::write(dir.join("l2"), data).unwrap();
    let data = indoc::indoc! {r#"
        # stub license
    "#};
    fs::write(dir.join("l3"), data).unwrap();

    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="testing for license usage"
        SLOT=0
        LICENSE="l1"
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", data).unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="testing for license usage"
        SLOT=0
        LICENSE="use? ( l1 ) l2"
        IUSE="use"
    "#};
    repo.create_ebuild_from_str("cat/pkg-2", data).unwrap();
    repo.create_ebuild("cat/pkg-3", &[]).unwrap();

    // all licenses
    cmd("pk repo license")
        .arg(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            repo
              l2: 1 pkg
              l1: 2 pkgs
        "})
        .stderr("")
        .success();

    // invalid, selected license
    cmd("pk repo license --licenses nonexistent")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr(contains("unknown license: nonexistent"))
        .failure()
        .code(2);

    // matching packages for license
    cmd("pk repo license --licenses l1")
        .arg(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            cat/pkg-1
            cat/pkg-2
        "})
        .stderr("")
        .success();

    // matching packages for multiple licenses
    cmd("pk repo license --licenses l1,l2")
        .arg(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            cat/pkg-1
            cat/pkg-2
        "})
        .stderr("")
        .success();

    // unused license
    cmd("pk repo license --licenses l3")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn multiple_repos() {
    let data = test_data();
    let repo1 = data.ebuild_repo("metadata").unwrap();
    let repo2 = data.ebuild_repo("qa-primary").unwrap();

    // fails if any repo has invalid pkgs
    cmd("pk repo license")
        .args([&repo1, &repo2])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure();

    // ignore invalid pkgs
    cmd("pk repo license -i")
        .args([&repo1, &repo2])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
