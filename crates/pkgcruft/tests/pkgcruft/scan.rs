use pkgcraft::repo::ebuild::temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::prelude::*;

#[test]
fn missing_target() {
    cmd("pkgcruft scan")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn stdin_targets() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd("pkgcruft scan -")
        .arg(t.path())
        .write_stdin("cat/pkg\n")
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn nonexistent_path_target() {
    cmd("pkgcruft scan path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(2);
}

#[test]
fn empty_ebuild_repo() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd("pkgcruft scan")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
