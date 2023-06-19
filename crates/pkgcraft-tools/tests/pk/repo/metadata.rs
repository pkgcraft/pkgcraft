use std::fs;

use pkgcraft::repo::ebuild_temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::prelude::*;

#[test]
fn missing_repo_arg() {
    cmd("pk repo metadata")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn nonexistent_repo() {
    cmd("pk repo metadata")
        .arg("path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn no_pkgs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(!t.path().join("metadata/md5-cache").exists());
}

#[test]
fn single() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/dep-1", &["EAPI=1"]).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    let path = t.path().join("metadata/md5-cache/cat/dep-1");
    assert!(path.exists());
    let orig_modified = fs::metadata(&path).unwrap().modified().unwrap();

    // running again won't change the cache
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(orig_modified, modified);

    // -f/--force will change the cache
    for s in ["-f", "--force"] {
        cmd("pk repo metadata")
            .arg(s)
            .arg(t.path())
            .assert()
            .stdout("")
            .stderr("")
            .success();

        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        assert_ne!(orig_modified, modified);
    }
}

#[test]
fn multiple() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/a-1", &["EAPI=7"]).unwrap();
    t.create_ebuild("cat/b-1", &["EAPI=8"]).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(t.path().join("metadata/md5-cache/cat/a-1").exists());
    assert!(t.path().join("metadata/md5-cache/cat/b-1").exists());
}

#[test]
fn pkg_with_invalid_eapi() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/a-1", &["EAPI=invalid"]).ok();
    t.create_ebuild("cat/b-1", &["EAPI=8"]).unwrap();
    cmd("pk repo metadata")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);

    assert!(!t.path().join("metadata/md5-cache/cat/a-1").exists());
    assert!(t.path().join("metadata/md5-cache/cat/b-1").exists());
}

#[test]
fn multiple_repos() {
    let t1 = TempRepo::new("test1", None, 0, None).unwrap();
    t1.create_ebuild("cat/a-1", &["EAPI=7"]).unwrap();
    let t2 = TempRepo::new("test2", None, 0, None).unwrap();
    t2.create_ebuild("cat/b-1", &["EAPI=8"]).unwrap();
    cmd("pk repo metadata")
        .args([t1.path(), t2.path()])
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(t1.path().join("metadata/md5-cache/cat/a-1").exists());
    assert!(t2.path().join("metadata/md5-cache/cat/b-1").exists());
}
