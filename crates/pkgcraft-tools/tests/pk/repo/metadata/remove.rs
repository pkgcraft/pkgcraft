use pkgcraft::repo::ebuild::cache::Cache;
use pkgcraft::repo::ebuild::temp::Repo as TempRepo;
use pkgcraft::repo::Repository;
use pkgcraft::test::cmd;
use tempfile::tempdir;

use crate::predicates::lines_contain;

#[test]
fn run() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_pkg("cat/a-1", &[]).unwrap();
    let path = t.metadata.cache().path();

    // generate cache
    cmd("pk repo metadata regen")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(path.exists());
    assert!(path.join("cat/a-1").exists());

    // remove cache
    cmd("pk repo metadata remove")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    assert!(!path.exists());

    // missing cache removal is ignored
    cmd("pk repo metadata remove")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();

    let dir = tempdir().unwrap();
    let cache_path = dir.path().to_str().unwrap();

    // external cache removal isn't supported
    cmd("pk repo metadata remove")
        .args(["-p", cache_path])
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr(lines_contain([format!("external cache: {cache_path}")]))
        .failure()
        .code(2);
}
