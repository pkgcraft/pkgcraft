use std::env;

use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::repo::ebuild::cache::Cache;
use predicates::str::contains;
use tempfile::tempdir;

use crate::cmd;

#[test]
fn run() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/pkg-1", &[]).unwrap();
    let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
    let path = repo.metadata().cache().path();

    // generate cache
    cmd("pk repo metadata regen")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(path.exists());
    assert!(path.join("cat/pkg-1").exists());

    // remove cache
    cmd("pk repo metadata remove")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(!path.exists());

    // missing cache removal is ignored
    cmd("pk repo metadata remove")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn current_dir() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/pkg-1", &[]).unwrap();
    let repo = config.add_repo(&temp).unwrap().into_ebuild().unwrap();
    let path = repo.metadata().cache().path();
    env::set_current_dir(&repo).unwrap();

    // generate cache
    cmd("pk repo metadata regen")
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(path.exists());
    assert!(path.join("cat/pkg-1").exists());

    // remove cache
    cmd("pk repo metadata remove")
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(!path.exists());

    // missing cache removal is ignored
    cmd("pk repo metadata remove")
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn external_unsupported() {
    let repo = EbuildRepoBuilder::new().build().unwrap();
    let dir = tempdir().unwrap();
    let cache_path = dir.path().to_str().unwrap();

    for opt in ["-p", "--path"] {
        cmd("pk repo metadata remove")
            .args([opt, cache_path])
            .arg(&repo)
            .assert()
            .stdout("")
            .stderr(contains(format!("removal unsupported for external cache: {cache_path}")))
            .failure()
            .code(2);
    }
}
