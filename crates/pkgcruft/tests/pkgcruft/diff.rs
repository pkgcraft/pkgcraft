use std::io::Write;

use pkgcraft::test::cmd;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::NamedTempFile;

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
    let output = String::from_utf8(output).unwrap();
    let output: Vec<_> = output.lines().collect();
    assert!(output.is_empty());
}

#[test]
fn output() {
    let old = indoc::indoc! {r#"
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg"},"message":"arch"}
        {"kind":"DependencyDeprecated","scope":{"Version":["cat/pkg-1-r2",null]},"message":"BDEPEND: cat/deprecated"}
        {"kind":"WhitespaceInvalid","scope":{"Version":["cat/pkg-1-r2",{"line":3,"column":28}]},"message":"character '\\u{2001}'"}
    "#};
    let new = indoc::indoc! {r#"
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg"},"message":"arch"}
        {"kind":"DependencyDeprecated","scope":{"Version":["cat/pkg-1-r2",null]},"message":"BDEPEND: cat/deprecated"}
        {"kind":"WhitespaceUnneeded","scope":{"Version":["cat/pkg-1-r2",{"line":3,"column":0}]},"message":"empty line"}
    "#};

    let mut old_file = NamedTempFile::new().unwrap();
    old_file.write_all(old.as_bytes()).unwrap();
    let mut new_file = NamedTempFile::new().unwrap();
    new_file.write_all(new.as_bytes()).unwrap();

    let expected = indoc::indoc! {r#"
        -cat/pkg-1-r2, line 3, column 28: WhitespaceInvalid: character '\u{2001}'
        +cat/pkg-1-r2, line 3: WhitespaceUnneeded: empty line
    "#};
    let expected: Vec<_> = expected.lines().collect();

    let output = cmd("pkgcruft diff")
        .args([old_file.path(), new_file.path()])
        .output()
        .unwrap()
        .stdout;
    let output = String::from_utf8(output).unwrap();
    let output: Vec<_> = output.lines().collect();
    assert_eq!(&output, &expected);
}
