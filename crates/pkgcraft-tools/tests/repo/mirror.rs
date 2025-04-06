use std::{env, fs};

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn nonexistent_repo() {
    cmd("pk repo mirror path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("unknown repo: path/to/nonexistent/repo"))
        .failure()
        .code(2);

    cmd("pk repo mirror nonexistent-repo-alias")
        .assert()
        .stdout("")
        .stderr(contains("unknown repo: nonexistent-repo-alias"))
        .failure()
        .code(2);
}

#[test]
fn ignore() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();

    // invalid pkgs log errors and cause failure by default
    cmd("pk repo mirror")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);

    // ignoring invalid pkgs entirely skips them
    for opt in ["-i", "--ignore"] {
        cmd("pk repo mirror")
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
    cmd("pk repo mirror")
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
    cmd("pk repo mirror")
        .assert()
        .stdout("")
        .stderr(contains("non-ebuild repo: ."))
        .failure()
        .code(2);

    // repo working directory
    // fails due to invalid pkgs
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();
    env::set_current_dir(repo).unwrap();
    cmd("pk repo mirror")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure();
}

#[test]
fn single_repo() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    let data = indoc::indoc! {r#"
        mirror1 https://mirror/1
        mirror2 https://mirror/2/a https://mirror/2/b
        mirror3 https:///mirror/3
    "#};
    fs::write(repo.path().join("profiles/thirdpartymirrors"), data).unwrap();

    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="testing for mirror usage"
        SRC_URI="mirror://mirror1/file"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", data).unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="testing for mirror usage"
        SRC_URI="use? ( mirror://mirror1/file1 ) mirror://mirror2/file2"
        SLOT=0
        IUSE="use"
    "#};
    repo.create_ebuild_from_str("cat/pkg-2", data).unwrap();
    repo.create_ebuild("cat/pkg-3", &[]).unwrap();

    // all mirrors
    cmd("pk repo mirror")
        .arg(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            repo
              mirror2: 1 pkg
              mirror1: 2 pkgs
        "})
        .stderr("")
        .success();

    // invalid, selected mirror
    cmd("pk repo mirror --mirror nonexistent")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr(contains("unknown mirror: nonexistent"))
        .failure()
        .code(2);

    // matching packages for mirror
    cmd("pk repo mirror --mirror mirror1")
        .arg(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            cat/pkg-1
            cat/pkg-2
        "})
        .stderr("")
        .success();

    // unused mirror
    cmd("pk repo mirror --mirror mirror3")
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
    cmd("pk repo mirror")
        .args([&repo1, &repo2])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure();

    // ignore invalid pkgs
    cmd("pk repo mirror -i")
        .args([&repo1, &repo2])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
