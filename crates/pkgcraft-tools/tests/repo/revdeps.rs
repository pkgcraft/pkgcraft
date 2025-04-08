use std::{env, fs};

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{assert_ordered_eq, cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;
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
    repo.create_ebuild("c/d-1", &[]).unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with revdeps"
        IUSE="u1 u2"
        SLOT=0
        DEPEND="a/b !c/d"
        RDEPEND="u1? ( a/b )"
        BDEPEND="!u1? ( a/b )"
        IDEPEND="u1? ( c/d ) u2? ( c/d )"
        PDEPEND="u1? ( a/b !u2? ( c/d ) )"
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    env::set_current_dir(repo.path()).unwrap();
    cmd("pk repo revdeps")
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // pull content from revdep files
    let file_to_list = |path: &str| -> Vec<String> {
        fs::read_to_string(path)
            .unwrap()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect()
    };

    assert_ordered_eq!(file_to_list("revdeps/depend/a/b"), ["cat/pkg-1"]);
    assert_ordered_eq!(file_to_list("revdeps/depend/c/d"), ["[B]cat/pkg-1"]);
    assert_ordered_eq!(file_to_list("revdeps/rdepend/a/b"), ["cat/pkg-1:u1"]);
    assert_ordered_eq!(file_to_list("revdeps/bdepend/a/b"), ["cat/pkg-1:!u1"]);
    assert_ordered_eq!(file_to_list("revdeps/idepend/c/d"), ["cat/pkg-1:u1", "cat/pkg-1:u2"]);
    assert_ordered_eq!(file_to_list("revdeps/pdepend/a/b"), ["cat/pkg-1:u1"]);
    assert_ordered_eq!(file_to_list("revdeps/pdepend/c/d"), ["cat/pkg-1:u1+!u2"]);
}
