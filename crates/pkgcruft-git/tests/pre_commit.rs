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
    Command::new(env!("CARGO_BIN_EXE_pkgcruft-git-pre-commit"))
        .arg("--invalid-option")
        .assert()
        .stdout("")
        .stderr(contains("pkgcruft-git-pre-commit"))
        .failure()
        .code(2);
}

#[tokio::test]
async fn invalid_repo() {
    let dir = tempdir().unwrap();
    let path = dir.path().to_str().unwrap();
    let git_repo = GitRepo::init(path).unwrap();

    // inject hook into repo
    let hook_path = git_repo.path().join("hooks/pre-commit");
    symlink(env!("CARGO_BIN_EXE_pkgcruft-git-pre-commit"), hook_path).unwrap();

    // trigger hook via `git commit`
    git!("commit -m test")
        .current_dir(path)
        .assert()
        .stdout("")
        .stderr(format!("pkgcruft-git-pre-commit: error: invalid ebuild repo: {path}\n"))
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

    // create package
    let data = indoc::indoc! {r#"
        EAPI=8

        DESCRIPTION="uncommitted package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-2", data).unwrap();
    // add package to index
    git_repo.stage(&["*"]).unwrap();

    // inject hook into repo
    let hook_path = git_repo.path().join("hooks/pre-commit");
    symlink(env!("CARGO_BIN_EXE_pkgcruft-git-pre-commit"), hook_path).unwrap();

    // trigger hook via `git commit`
    git!("commit -m test")
        .current_dir(&repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[tokio::test]
async fn failure() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    let dir = repo.path().join("licenses");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("abc"), "stub license").unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8

        DESCRIPTION="test git pre-commit hook"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", data).unwrap();

    // initialize git repo
    let git_repo = GitRepo::init(&repo).unwrap();
    let oid = git_repo.stage(&["*"]).unwrap();
    git_repo.commit(oid, "initial import").unwrap();

    // create package
    let data = indoc::indoc! {r#"
        DESCRIPTION="uncommitted package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-2", data).unwrap();
    // add package to index
    git_repo.stage(&["*"]).unwrap();

    // inject hook into repo
    let hook_path = git_repo.path().join("hooks/pre-commit");
    symlink(env!("CARGO_BIN_EXE_pkgcruft-git-pre-commit"), hook_path).unwrap();

    // trigger hook via `git commit`
    git!("commit -m test")
        .current_dir(&repo)
        .assert()
        .stdout("")
        .stderr(indoc::indoc! {"
            cat/pkg
              MetadataError: version 2: unsupported EAPI: 0
            pkgcruft-git-pre-commit: error: scanning errors found
        "})
        .failure()
        .code(1);
}
