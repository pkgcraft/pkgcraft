use std::{env, fs};

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    cmd("pkgcruft ignore")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn current_dir_targets() {
    // empty dir
    let tmpdir = tempdir().unwrap();
    let path = tmpdir.path().to_str().unwrap();
    env::set_current_dir(path).unwrap();
    cmd("pkgcruft ignore")
        .assert()
        .stdout("")
        .stderr(contains("invalid ebuild repo: ."))
        .failure()
        .code(2);

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        # pkgcruft-ignore: PythonUpdate
        EAPI=8
        DESCRIPTION="ebuild with ignore directive"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat-a/pkg-a-1", &data).unwrap();
    let data = indoc::formatdoc! {r#"
        # pkgcruft-ignore: PythonUpdate
        EAPI=8
        DESCRIPTION="ebuild with ignore directive"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat-a/pkg-b-1", &data).unwrap();
    let data = indoc::formatdoc! {r#"
        # pkgcruft-ignore: PythonUpdate
        EAPI=8
        DESCRIPTION="ebuild with ignore directive"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat-b/pkg-c-1", &data).unwrap();

    let data = indoc::indoc! {"
        # ignore style reports at repo level
        @style
    "};
    fs::write(repo.path().join(".pkgcruft-ignore"), data).unwrap();
    let data = indoc::indoc! {"
        # ignore Manifest check reports at category level
        @Manifest
    "};
    fs::write(repo.path().join("cat-a/.pkgcruft-ignore"), data).unwrap();
    let data = indoc::indoc! {"
        # ignore UnstableOnly at package level
        UnstableOnly
    "};
    fs::write(repo.path().join("cat-a/pkg-a/.pkgcruft-ignore"), data).unwrap();

    // repo dir
    env::set_current_dir(&repo).unwrap();
    cmd("pkgcruft ignore")
        .assert()
        .stdout(indoc::indoc! {"
            test
              @style
            cat-a/*
              @Manifest
            cat-a/pkg-a/*
              UnstableOnly
            cat-a/pkg-a/pkg-a-1.ebuild
              PythonUpdate
            cat-a/pkg-b/pkg-b-1.ebuild
              PythonUpdate
            cat-b/pkg-c/pkg-c-1.ebuild
              PythonUpdate
        "})
        .stderr("")
        .success();

    // category dir
    env::set_current_dir(repo.path().join("cat-a")).unwrap();
    cmd("pkgcruft ignore")
        .assert()
        .stdout(indoc::indoc! {"
            test
              @style
            cat-a/*
              @Manifest
            cat-a/pkg-a/*
              UnstableOnly
            cat-a/pkg-a/pkg-a-1.ebuild
              PythonUpdate
            cat-a/pkg-b/pkg-b-1.ebuild
              PythonUpdate
        "})
        .stderr("")
        .success();

    // package dir
    env::set_current_dir(repo.path().join("cat-a/pkg-a")).unwrap();
    cmd("pkgcruft ignore")
        .assert()
        .stdout(indoc::indoc! {"
            test
              @style
            cat-a/*
              @Manifest
            cat-a/pkg-a/*
              UnstableOnly
            cat-a/pkg-a/pkg-a-1.ebuild
              PythonUpdate
        "})
        .stderr("")
        .success();
}
