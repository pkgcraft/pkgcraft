use pkgcraft::repo::ebuild_temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::prelude::*;

#[test]
fn missing_repo_arg() {
    cmd(&format!("pk repo eapis"))
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn nonexistent_repo() {
    cmd(&format!("pk repo eapis path/to/nonexistent/repo"))
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn no_pkgs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd(&format!("pk repo eapis {}", t.path()))
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn single() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/dep-1", &["EAPI=1"]).unwrap();
    cmd(&format!("pk repo eapis {}", t.path()))
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn multiple() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/a-1", &["EAPI=7"]).unwrap();
    t.create_ebuild("cat/b-1", &["EAPI=8"]).unwrap();
    cmd(&format!("pk repo eapis {}", t.path()))
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn multiple_repos() {
    let t1 = TempRepo::new("test1", None, 0, None).unwrap();
    t1.create_ebuild("cat/a-1", &["EAPI=7"]).unwrap();
    let t2 = TempRepo::new("test2", None, 0, None).unwrap();
    t2.create_ebuild("cat/b-1", &["EAPI=8"]).unwrap();
    cmd(&format!("pk repo eapis {} {}", t1.path(), t2.path()))
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
