use pkgcraft::repo::ebuild_temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::prelude::*;

#[test]
fn invalid_cwd_target() {
    cmd("pk pkg source")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn nonexistent_target() {
    cmd("pk pkg source path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn no_pkgs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd(format!("pk pkg source {}", t.path()))
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn single() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/dep-1", &[]).unwrap();
    cmd(format!("pk pkg source {}", t.path()))
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn single_from_stdin() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/dep-1", &[]).unwrap();
    cmd(format!("pk pkg source -r {} -", t.path()))
        .write_stdin("cat/dep")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
