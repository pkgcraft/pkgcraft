use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::Cache;
use pkgcraft::repo::Repository;
use pkgcraft::test::cmd;
use tempfile::tempdir;

use crate::predicates::lines_contain;

#[test]
fn run() {
    let mut config = Config::default();
    let mut temp = config.temp_repo("test", 0, None).unwrap();
    temp.create_ebuild("cat/a-1", &[]).unwrap();
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();
    let path = repo.metadata().cache().path();

    // generate cache
    cmd("pk repo metadata regen")
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(path.exists());
    assert!(path.join("cat/a-1").exists());

    // remove cache
    cmd("pk repo metadata remove")
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(!path.exists());

    // missing cache removal is ignored
    cmd("pk repo metadata remove")
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    let dir = tempdir().unwrap();
    let cache_path = dir.path().to_str().unwrap();

    // external cache removal isn't supported
    cmd("pk repo metadata remove")
        .args(["-p", cache_path])
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr(lines_contain([format!("external cache: {cache_path}")]))
        .failure()
        .code(2);
}
