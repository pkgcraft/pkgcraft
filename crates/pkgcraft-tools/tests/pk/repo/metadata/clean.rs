use std::fs;

use pkgcraft::repo::ebuild::cache::Cache;
use pkgcraft::repo::ebuild::temp::Repo as TempRepo;
use pkgcraft::repo::Repository;
use pkgcraft::test::cmd;

#[test]
fn run() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_pkg("a/b-1", &[]).unwrap();
    t.create_pkg("cat/a-1", &[]).unwrap();
    t.create_pkg("cat/b-1", &[]).unwrap();
    t.create_pkg("cat/b-2", &[]).unwrap();
    let path = t.repo().metadata().cache().path();

    // generate cache
    cmd("pk repo metadata regen")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(path.join("a/b-1").exists());
    assert!(path.join("cat/a-1").exists());
    assert!(path.join("cat/b-1").exists());
    assert!(path.join("cat/b-2").exists());

    // create old and temp files
    fs::write(path.join("cat/a-0"), "").unwrap();
    fs::write(path.join("cat/.a-1"), "").unwrap();

    // no outdated entries removes only unrelated files
    cmd("pk repo metadata clean")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(path.join("a/b-1").exists());
    assert!(path.join("cat/a-1").exists());
    assert!(path.join("cat/b-1").exists());
    assert!(path.join("cat/b-2").exists());
    assert!(!path.join("cat/a-0").exists());
    assert!(!path.join("cat/.a-1").exists());

    // remove pkgs and create old and temp files
    fs::write(path.join("cat/a-0"), "").unwrap();
    fs::write(path.join("cat/.a-1"), "").unwrap();
    fs::remove_dir_all(t.repo().path().join("cat/b")).unwrap();
    fs::remove_dir_all(t.repo().path().join("a")).unwrap();

    // outdated cache files and directories are removed
    cmd("pk repo metadata clean")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(!path.join("a").exists());
    assert!(path.join("cat/a-1").exists());
    assert!(!path.join("cat/b-1").exists());
    assert!(!path.join("cat/b-2").exists());
    assert!(!path.join("cat/a-0").exists());
    assert!(!path.join("cat/.a-1").exists());
}
