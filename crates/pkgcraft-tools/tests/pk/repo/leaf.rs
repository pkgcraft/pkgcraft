use pkgcraft::repo::ebuild_temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::prelude::*;

#[test]
fn missing_repo_arg() {
    cmd(&format!("pk repo leaf"))
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn nonexistent_repo() {
    cmd(&format!("pk repo leaf path/to/nonexistent/repo"))
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn multiple_repos_not_supported() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd(&format!("pk repo leaf {} {}", t.path(), t.path()))
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn no_pkgs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd(&format!("pk repo leaf {}", t.path()))
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn single() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/dep-1", &[]).unwrap();
    t.create_ebuild("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    cmd(&format!("pk repo leaf {}", t.path()))
        .assert()
        .stdout("cat/leaf-1\n")
        .stderr("")
        .success();
}

#[test]
fn multiple() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/dep-1", &[]).unwrap();
    t.create_ebuild("cat/leaf-1", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    t.create_ebuild("cat/leaf-2", &["DEPEND=>=cat/dep-1"])
        .unwrap();
    cmd(&format!("pk repo leaf {}", t.path()))
        .assert()
        .stdout("cat/leaf-1\ncat/leaf-2\n")
        .stderr("")
        .success();
}

#[test]
fn none() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/a-1", &["DEPEND=>=cat/b-1"]).unwrap();
    t.create_ebuild("cat/b-1", &["DEPEND=>=cat/a-1"]).unwrap();
    cmd(&format!("pk repo leaf {}", t.path()))
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
