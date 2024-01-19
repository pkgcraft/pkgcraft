use pkgcraft::repo::ebuild::temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::str::contains;

#[test]
fn invalid_cwd() {
    cmd("pkgcruft scan")
        .assert()
        .stdout("")
        .stderr(contains("invalid repo path"))
        .failure()
        .code(2);
}

#[test]
fn nonexistent_path_target() {
    cmd("pkgcruft scan path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("invalid path target"))
        .failure()
        .code(2);
}

#[test]
fn invalid_path_target() {
    cmd("pkgcruft scan /")
        .assert()
        .stdout("")
        .stderr(contains("invalid repo path"))
        .failure()
        .code(2);
}

#[test]
fn invalid_dep_restricts() {
    for s in ["^pkg", "cat&pkg"] {
        cmd("pkgcruft scan")
            .arg(s)
            .assert()
            .stdout("")
            .stderr(contains(format!("invalid dep restriction: {s}")))
            .failure()
            .code(2);
    }
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
fn empty_ebuild_repo() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd("pkgcruft scan")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
