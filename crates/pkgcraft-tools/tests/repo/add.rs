use pkgcraft::test::cmd;
use predicates::str::contains;

#[test]
fn unsupported_syncers() {
    cmd("pk repo add nonexistent")
        .assert()
        .stdout("")
        .stderr(contains("no syncers available: nonexistent"))
        .failure()
        .code(2);
}

#[test]
fn nonexistent_local_repo() {
    cmd("pk repo add /path/to/repo")
        .assert()
        .stdout("")
        .stderr(contains("invalid local repo: /path/to/repo: No such file or directory"))
        .failure()
        .code(2);
}
