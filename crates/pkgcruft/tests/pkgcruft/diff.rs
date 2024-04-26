use pkgcraft::test::cmd;
use predicates::str::contains;

#[test]
fn nonexistent_path_targets() {
    cmd("pkgcruft diff path/to/nonexistent/file1.json path/to/nonexistent/file2.json")
        .assert()
        .stdout("")
        .stderr(contains("failed loading file"))
        .failure()
        .code(2);
}
