use std::fs;
use std::os::unix::fs::symlink;

use assert_cmd::Command;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::tempdir;

use crate::git::{GitRepo, git};

#[tokio::test]
async fn invalid_command() {
    Command::new(env!("CARGO_BIN_EXE_pkgcruft-git-prepare-commit-msg"))
        .arg("--invalid-option")
        .assert()
        .stdout("")
        .stderr(contains("pkgcruft-git-prepare-commit-msg"))
        .failure()
        .code(2);
}

#[tokio::test]
async fn invalid_repo() {
    let dir = tempdir().unwrap();
    let path = dir.path();
    let git_repo = GitRepo::init(path).unwrap();

    // stage content for commit
    fs::write(path.join("data"), "data").unwrap();
    git_repo.stage(&["*"]).unwrap();

    // inject hook into repo
    let hook_path = git_repo.path().join("hooks/prepare-commit-msg");
    symlink(env!("CARGO_BIN_EXE_pkgcruft-git-prepare-commit-msg"), hook_path).unwrap();

    // trigger hook via `git commit`
    git!("commit")
        .current_dir(path)
        .assert()
        .stdout("")
        .stderr(contains("pkgcruft-git-prepare-commit-msg: error: invalid ebuild repo: "))
        .failure()
        .code(1);
}

#[tokio::test]
async fn success() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    let dir = repo.path().join("licenses");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("abc"), "stub license").unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8

        DESCRIPTION="committed package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", data).unwrap();

    // initialize git repo
    let git_repo = GitRepo::init(&repo).unwrap();
    let oid = git_repo.stage(&["*"]).unwrap();
    git_repo.commit(oid, "initial import").unwrap();

    // inject hook into repo
    let hook_path = git_repo.path().join("hooks/prepare-commit-msg");
    symlink(env!("CARGO_BIN_EXE_pkgcruft-git-prepare-commit-msg"), hook_path).unwrap();

    // add package to index
    let data = indoc::indoc! {r#"
        EAPI=8

        DESCRIPTION="uncommitted package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-2", data).unwrap();
    git_repo.stage(&["*"]).unwrap();

    // trigger hook via `git commit`
    git!("commit")
        .env("GIT_EDITOR", "sed -i '1s/$/summary/'")
        .current_dir(&repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // verify commit message content
    let head = git_repo.head().unwrap();
    let commit = head.peel_to_commit().unwrap();
    assert_eq!(commit.message().unwrap(), "cat/pkg: summary\n");
}

#[tokio::test]
async fn existing_message() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    let dir = repo.path().join("licenses");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("abc"), "stub license").unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8

        DESCRIPTION="committed package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", data).unwrap();

    // initialize git repo
    let git_repo = GitRepo::init(&repo).unwrap();
    let oid = git_repo.stage(&["*"]).unwrap();
    git_repo.commit(oid, "initial import").unwrap();

    // inject hook into repo
    let hook_path = git_repo.path().join("hooks/prepare-commit-msg");
    symlink(env!("CARGO_BIN_EXE_pkgcruft-git-prepare-commit-msg"), hook_path).unwrap();

    // add package to index
    let data = indoc::indoc! {r#"
        EAPI=8

        DESCRIPTION="uncommitted package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-3", data).unwrap();
    git_repo.stage(&["*"]).unwrap();

    // trigger hook via `git commit`
    git!("commit")
        .current_dir(&repo)
        .args(["-m", "add new version"])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // message generation skipped when a message is provided
    let head = git_repo.head().unwrap();
    let commit = head.peel_to_commit().unwrap();
    assert_eq!(commit.message().unwrap(), "add new version\n");
}

#[tokio::test]
async fn multiple_pkgs() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    let dir = repo.path().join("licenses");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("abc"), "stub license").unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8

        DESCRIPTION="committed package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", data).unwrap();

    // initialize git repo
    let git_repo = GitRepo::init(&repo).unwrap();
    let oid = git_repo.stage(&["*"]).unwrap();
    git_repo.commit(oid, "initial import").unwrap();

    // inject hook into repo
    let hook_path = git_repo.path().join("hooks/prepare-commit-msg");
    symlink(env!("CARGO_BIN_EXE_pkgcruft-git-prepare-commit-msg"), hook_path).unwrap();

    // add packages to index
    let data = indoc::indoc! {r#"
        EAPI=8

        DESCRIPTION="uncommitted package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("a/pkg-1", data).unwrap();
    repo.create_ebuild_from_str("b/pkg-1", data).unwrap();
    git_repo.stage(&["*"]).unwrap();

    // trigger hook via `git commit`
    git!("commit")
        .env("GIT_EDITOR", "sed -i '1s/$/summary/'")
        .current_dir(&repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // message generation skipped for multiple pkgs
    let head = git_repo.head().unwrap();
    let commit = head.peel_to_commit().unwrap();
    assert_eq!(commit.message().unwrap(), "summary\n");
}
