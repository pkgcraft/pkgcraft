use pkgcraft::test::cmd;
use predicates::str::contains;

use crate::replay::QA_PRIMARY_FILE;

#[test]
fn nonexistent_path_targets() {
    cmd("pkgcruft diff path/to/nonexistent/file1.json path/to/nonexistent/file2.json")
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
