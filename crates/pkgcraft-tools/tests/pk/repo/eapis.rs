use pkgcraft::repo::ebuild::temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::prelude::*;

use crate::predicates::lines_contain;

#[test]
fn missing_repo_arg() {
    cmd("pk repo eapis")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn nonexistent_repo() {
    cmd("pk repo eapis path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn no_pkgs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd("pk repo eapis")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn single() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat/dep-1", &["EAPI=8"]).unwrap();
    cmd("pk repo eapis")
        .arg(t.path())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn multiple() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat/a-1", &["EAPI=7"]).unwrap();
    t.create_raw_pkg("cat/b-1", &["EAPI=8"]).unwrap();
    cmd("pk repo eapis")
        .arg(t.path())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn multiple_repos() {
    let t1 = TempRepo::new("test1", None, 0, None).unwrap();
    t1.create_raw_pkg("cat/a-1", &["EAPI=7"]).unwrap();
    let t2 = TempRepo::new("test2", None, 0, None).unwrap();
    t2.create_raw_pkg("cat/b-1", &["EAPI=8"]).unwrap();
    cmd("pk repo eapis")
        .args([t1.path(), t2.path()])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn option_eapi() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("a/b-1", &["EAPI=8"]).unwrap();
    t.create_raw_pkg("cat/pkg-2", &["EAPI=8"]).unwrap();

    // invalid EAPI
    cmd("pk repo eapis --eapi nonexistent")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);

    // matching packages
    cmd("pk repo eapis --eapi 8")
        .arg(t.path())
        .assert()
        .stdout(lines_contain(["a/b-1", "cat/pkg-2"]))
        .stderr("")
        .success();

    // no matching packages
    cmd("pk repo eapis --eapi 7")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
