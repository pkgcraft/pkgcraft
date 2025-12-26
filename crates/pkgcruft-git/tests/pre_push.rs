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
    repo.create_ebuild_from_str("a/b-1", data).unwrap();

    // initialize git repo
    let git_repo = GitRepo::init(&repo).unwrap();
    let oid = git_repo.stage(&["*"]).unwrap();
    git_repo.commit(oid, "initial import").unwrap();

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

    // create good eclass
    let data = indoc::indoc! {r#"
        # stub eclass
        DEPEND="a/b"
    "#};
    repo.create_eclass("e1", data).unwrap();
    // create package
    let data = indoc::indoc! {r#"
        EAPI=8

        inherit e1

        DESCRIPTION="committed package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("a/b-2", data).unwrap();

    // add commit to client repo
    git_repo.stage(&["eclass", "a/b"]).unwrap();
    git!("commit -m good").current_dir(&repo).assert().success();

    // trigger hook via `git push`
    git!("push")
        .current_dir(&repo)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .success();

    // create bad package
    let data = indoc::indoc! {r#"
        DESCRIPTION="package with unsupported EAPI"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-2", data).unwrap();

    // add commit to client repo
    git_repo.stage(&["cat/pkg"]).unwrap();
    git!("commit -m bad-pkg")
        .current_dir(&repo)
        .assert()
        .success();

    // trigger hook via `git push`
    git!("push")
        .current_dir(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            cat/pkg
              MetadataError: version 2: unsupported EAPI: 0
        "})
        .stderr(contains("pkgcruft-git-pre-push: error: scanning errors found"))
        .failure()
        .code(1);

    // create bad eclass
    let data = indoc::indoc! {r#"
        # stub eclass
        cd path
    "#};
    repo.create_eclass("e1", data).unwrap();

    // add commit to client repo
    git_repo.stage(&["eclass"]).unwrap();
    git!("commit -m bad-eclass")
        .current_dir(&repo)
        .assert()
        .success();

    // trigger hook via `git push`
    git!("push")
        .current_dir(&repo)
        .assert()
        .stdout(indoc::indoc! {"
            a/b
              MetadataError: version 2: line 3: inherit: error: failed loading eclass: e1: line 2: disabled builtin: cd

            cat/pkg
              MetadataError: version 2: unsupported EAPI: 0
        "})
        .stderr(contains("pkgcruft-git-pre-push: error: scanning errors found"))
        .failure()
        .code(1);
}
