use std::fs;
use std::os::unix::fs::symlink;

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::tempdir;

use crate::git::{GitRepo, git};

#[tokio::test]
async fn hook() {
    // create bare remote repo
    let remote_dir = tempdir().unwrap();
    let remote_path = remote_dir.path().to_str().unwrap();
    GitRepo::init_bare(remote_path).unwrap();

    // create client repo
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    let licenses_dir = repo.path().join("licenses");
    fs::create_dir(&licenses_dir).unwrap();
    fs::write(licenses_dir.join("abc"), "stub license").unwrap();
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

    // add remote and push
    git!("remote add origin")
        .current_dir(&repo)
        .arg(remote_path)
        .assert()
        .success();
    git!("push -u origin main")
        .current_dir(&repo)
        .assert()
        .success();

    // inject hook into client repo
    let hook_path = git_repo.path().join("hooks/pre-push");
    symlink(env!("CARGO_BIN_EXE_pkgcruft-git-pre-push"), hook_path).unwrap();

    // add good commit to client repo
    repo.create_ebuild_from_str("cat/pkg-2", data).unwrap();
    git_repo.stage(&["cat/pkg"]).unwrap();
    git!("commit -m good").current_dir(&repo).assert().success();

    // trigger hook via `git push`
    git!("push")
        .current_dir(&repo)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .success();

    // add bad commit to client repo
    let data = indoc::indoc! {r#"
        DESCRIPTION="bad package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-2", data).unwrap();
    git_repo.stage(&["cat/pkg"]).unwrap();
    git!("commit -m bad").current_dir(&repo).assert().success();

    // trigger hook via `git push`
    git!("push")
        .current_dir(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            cat/pkg
              MetadataError: version 2: unsupported EAPI: 0
        "})
        .stderr(contains("Error: scanning errors found"))
        .failure()
        .code(1);
}
