use std::io::Write;

use pkgcraft::repo::Repository;
use pkgcraft::test::{cmd, TEST_DATA};
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::NamedTempFile;

#[test]
fn nonexistent_path_target() {
    cmd("pkgcruft replay path/to/nonexistent/file.json")
        .assert()
        .stdout("")
        .stderr(contains("failed loading file"))
        .failure()
        .code(2);
}

#[test]
fn invalid_dir_target() {
    cmd("pkgcruft replay .")
        .assert()
        .stdout("")
        .stderr(contains("failed reading line: Is a directory"))
        .failure()
        .code(2);
}

#[test]
fn stdin() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();

    // run scan, saving serialized report output
    let output = cmd("pkgcruft scan -R json")
        .arg(repo.path())
        .output()
        .unwrap()
        .stdout;

    // valid
    cmd("pkgcruft replay -")
        .write_stdin(output)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // invalid
    cmd("pkgcruft replay -")
        .write_stdin("invalid serialized report\n")
        .assert()
        .stdout("")
        .stderr(contains("failed deserializing report"))
        .failure()
        .code(2);
}

#[test]
fn file_targets() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "invalid reports json file").unwrap();

    // invalid
    cmd("pkgcruft replay")
        .arg(file.path())
        .assert()
        .stdout("")
        .stderr(contains("failed deserializing report"))
        .failure()
        .code(2);

    // valid
    let mut file = NamedTempFile::new().unwrap();
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();

    // serialize reports to file
    let output = cmd("pkgcruft scan -R json")
        .arg(repo.path())
        .output()
        .unwrap()
        .stdout;
    file.write_all(&output).unwrap();

    // replay reports from file
    cmd("pkgcruft replay")
        .arg(file.path())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
