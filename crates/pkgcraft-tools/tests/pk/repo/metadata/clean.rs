use std::fs;

use pkgcraft::repo::ebuild::cache::Cache;
use pkgcraft::repo::ebuild::temp::EbuildTempRepo;
use pkgcraft::repo::Repository;
use pkgcraft::test::cmd;

#[test]
fn run() {
    let mut temp = EbuildTempRepo::new("test", None, 0, None).unwrap();
    temp.create_pkg("a/b-1", &[]).unwrap();
    temp.create_pkg("cat/a-1", &[]).unwrap();
    temp.create_pkg("cat/b-1", &[]).unwrap();
    temp.create_pkg("cat/b-2", &[]).unwrap();
    let path = temp.metadata().cache().path();

    // generate cache
    cmd("pk repo metadata regen")
        .arg(temp.path())
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
        .arg(temp.path())
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
    fs::remove_dir_all(temp.path().join("cat/b")).unwrap();
    fs::remove_dir_all(temp.path().join("a")).unwrap();

    // outdated cache files and directories are removed
    cmd("pk repo metadata clean")
        .arg(temp.path())
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
