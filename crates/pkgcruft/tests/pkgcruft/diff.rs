use pkgcraft::test::cmd;
use predicates::str::contains;

use crate::replay::QA_PRIMARY_FILE;

#[test]
fn missing_args() {
    // missing both file args
    cmd("pkgcruft diff")
        .assert()
        .stdout("")
        .stderr(contains("OLD"))
        .failure()
        .code(2);

    // missing second file arg
    cmd("pkgcruft diff file1.json")
        .assert()
        .stdout("")
        .stderr(contains("NEW"))
        .failure()
        .code(2);
}

#[test]
fn nonexistent_files() {
    // first
    cmd("pkgcruft diff")
        .arg("path/to/nonexistent/file1.json")
        .arg(QA_PRIMARY_FILE.path())
        .assert()
        .stdout("")
        .stderr(contains("failed loading file"))
        .failure()
        .code(2);

    // second
    cmd("pkgcruft diff")
        .arg(QA_PRIMARY_FILE.path())
        .arg("path/to/nonexistent/file1.json")
        .assert()
        .stdout("")
        .stderr(contains("failed loading file"))
        .failure()
        .code(2);

    // both
    cmd("pkgcruft diff")
        .args(["path/to/nonexistent/file1.json", "path/to/nonexistent/file2.json"])
        .assert()
        .stdout("")
        .stderr(contains("failed loading file"))
        .failure()
        .code(2);
}

#[test]
fn empty() {
    let output = cmd("pkgcruft diff")
        .args([QA_PRIMARY_FILE.path(), QA_PRIMARY_FILE.path()])
        .output()
        .unwrap()
        .stdout;
    let data = String::from_utf8(output).unwrap();
    let data: Vec<_> = data.lines().collect();
    assert!(data.is_empty());
}
