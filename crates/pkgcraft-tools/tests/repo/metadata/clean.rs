use std::fs;

use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::Cache;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::cmd;

#[test]
fn run() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("a/b-1", &[]).unwrap();
    temp.create_ebuild("cat/a-1", &[]).unwrap();
    temp.create_ebuild("cat/b-1", &[]).unwrap();
    temp.create_ebuild("cat/b-2", &[]).unwrap();
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();
    let path = repo.metadata().cache().path();

    // generate cache
    cmd("pk repo metadata regen")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(path.join("a/b-1").exists());
    assert!(path.join("cat/a-1").exists());
    assert!(path.join("cat/b-1").exists());
    assert!(path.join("cat/b-2").exists());

    // create old, temp, and extraneous files
    fs::write(path.join("cat/a-0"), "").unwrap();
    fs::write(path.join("cat/.a-1"), "").unwrap();
    fs::write(path.join("cat/.random"), "").unwrap();
    fs::write(path.join("cat/random"), "").unwrap();

    // no outdated entries removes only unrelated files
    cmd("pk repo metadata clean")
        .arg(&repo)
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
    assert!(!path.join("cat/.random").exists());
    assert!(!path.join("cat/random").exists());

    // remove pkgs and create old, temp, and extraneous files
    fs::write(path.join("cat/a-0"), "").unwrap();
    fs::write(path.join("cat/.a-1"), "").unwrap();
    fs::write(path.join("cat/.random"), "").unwrap();
    fs::write(path.join("cat/random"), "").unwrap();
    fs::remove_dir_all(repo.path().join("cat/b")).unwrap();
    fs::remove_dir_all(repo.path().join("a")).unwrap();

    // outdated cache files and directories are removed
    cmd("pk repo metadata clean")
        .arg(&repo)
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
    assert!(!path.join("cat/.random").exists());
    assert!(!path.join("cat/random").exists());
}
