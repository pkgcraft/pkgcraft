use pkgcraft::test::cmd;
use predicates::str::contains;

#[test]
fn nonexistent_repo() {
    cmd("pk repo sync nonexistent")
        .assert()
        .stdout("")
        .stderr(contains("nonexistent repo: nonexistent"))
        .failure()
        .code(2);
}

#[test]
fn no_repos() {
    cmd("pk repo sync").assert().stdout("").stderr("").success();
}
