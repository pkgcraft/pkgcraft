use std::fs;
use std::io::Write;

use itertools::Itertools;
use pkgcraft::test::*;
use pkgcruft::test::*;
use predicates::prelude::*;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::NamedTempFile;

/// Temporary file of all serialized reports from the primary QA test repo.
pub(crate) fn qa_primary_file() -> NamedTempFile {
    let data = test_data();
    let repo = data.path().join("repos/valid/qa-primary");
    let reports = glob_reports!("{repo}/**/reports.json");
    let data = reports.iter().map(|x| x.to_json()).join("\n");
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(data.as_bytes()).unwrap();
    file
}

#[test]
fn missing_target() {
    cmd("pkgcruft replay")
        .assert()
        .stdout("")
        .stderr(contains("FILE"))
        .failure()
        .code(2);
}

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
    let file = qa_primary_file();

    // valid
    cmd("pkgcruft replay -")
        .write_stdin(fs::read_to_string(file.path()).unwrap())
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

    let file = qa_primary_file();

    // valid
    cmd("pkgcruft replay")
        .arg(file.path())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // multiple
    cmd("pkgcruft replay")
        .args([file.path(), file.path()])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn checks() {
    let file = qa_primary_file();
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();
    let repo = repo.path();
    let single_expected = glob_reports!("{repo}/Dependency/**/reports.json");
    let multiple_expected = glob_reports!(
        "{repo}/Dependency/**/reports.json",
        "{repo}/EapiStatus/**/reports.json",
        "{repo}/Keywords/**/reports.json",
    );
    let data = multiple_expected.iter().map(|x| x.to_json()).join("\n");

    // invalid
    cmd("pkgcruft replay -r @invalid")
        .arg(file.path())
        .assert()
        .stdout("")
        .stderr(contains("invalid report set: invalid"))
        .failure()
        .code(2);

    // single
    let reports = cmd("pkgcruft replay -R json -r @Dependency -")
        .write_stdin(data.as_str())
        .to_reports()
        .unwrap();
    assert_eq!(&single_expected, &reports);

    // multiple
    let reports = cmd("pkgcruft replay -R json -r @Dependency,@EapiStatus,@Keywords -")
        .write_stdin(data.as_str())
        .to_reports()
        .unwrap();
    assert_eq!(&multiple_expected, &reports);
}

#[test]
fn levels() {
    let file = qa_primary_file();
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();
    let repo = repo.path();
    let single_expected = glob_reports!("{repo}/EapiStatus/EapiDeprecated/reports.json");
    let multiple_expected = glob_reports!("{repo}/EapiStatus/**/reports.json");
    let data = multiple_expected.iter().map(|x| x.to_json()).join("\n");

    // invalid
    cmd("pkgcruft replay -r @invalid")
        .arg(file.path())
        .assert()
        .stdout("")
        .stderr(contains("invalid report set: invalid"))
        .failure()
        .code(2);

    // single
    let reports = cmd("pkgcruft replay -R json -r @warning -")
        .write_stdin(data.as_str())
        .to_reports()
        .unwrap();
    assert_eq!(&single_expected, &reports);

    // multiple
    let reports = cmd("pkgcruft replay -R json -r @warning,@error -")
        .write_stdin(data.as_str())
        .to_reports()
        .unwrap();
    assert_eq!(&multiple_expected, &reports);
}

#[test]
fn reports() {
    let file = qa_primary_file();
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();
    let repo = repo.path();
    let single_expected = glob_reports!("{repo}/Dependency/DependencyDeprecated/reports.json");
    let multiple_expected = glob_reports!(
        "{repo}/Dependency/DependencyDeprecated/reports.json",
        "{repo}/EapiStatus/EapiBanned/reports.json",
        "{repo}/Keywords/KeywordsUnsorted/reports.json",
    );
    let data = multiple_expected.iter().map(|x| x.to_json()).join("\n");

    for opt in ["-r", "--reports"] {
        // invalid
        cmd("pkgcruft replay")
            .args([opt, "invalid"])
            .arg(file.path())
            .assert()
            .stdout("")
            .stderr(contains("--reports"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft replay -R json -")
            .args([opt, "DependencyDeprecated"])
            .write_stdin(data.as_str())
            .to_reports()
            .unwrap();
        assert_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft replay -R json -")
            .args([opt, "DependencyDeprecated,EapiBanned,KeywordsUnsorted"])
            .write_stdin(data.as_str())
            .to_reports()
            .unwrap();
        assert_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn scopes() {
    let file = qa_primary_file();
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();
    let repo = repo.path();
    let single_expected = glob_reports!("{repo}/Dependency/DependencyDeprecated/reports.json");
    let multiple_expected = glob_reports!(
        "{repo}/Dependency/DependencyDeprecated/reports.json",
        "{repo}/Filesdir/FilesUnused/reports.json",
    );
    let data = multiple_expected.iter().map(|x| x.to_json()).join("\n");

    for opt in ["-s", "--scopes"] {
        // invalid
        cmd("pkgcruft replay")
            .args([opt, "invalid"])
            .arg(file.path())
            .assert()
            .stdout("")
            .stderr(contains("--scopes"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft replay -R json -")
            .args([opt, "version"])
            .write_stdin(data.as_str())
            .to_reports()
            .unwrap();
        assert_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft replay -R json -")
            .args([opt, "version,package"])
            .write_stdin(data.as_str())
            .to_reports()
            .unwrap();
        assert_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn pkgs() {
    let file = qa_primary_file();
    let reports = indoc::indoc! {r#"
        {"kind":"KeywordsDropped","scope":{"Version":["sys-fs/lvm2-2.03.22-r2",null]},"message":"alpha, hppa, ia64, m68k, ppc"}
        {"kind":"KeywordsDropped","scope":{"Version":["x11-wm/mutter-45.1",null]},"message":"ppc64"}
        {"kind":"UnstableOnly","scope":{"Package":"x11-wm/mutter"},"message":"arm, ppc64"}
    "#};
    let expected: Vec<_> = reports.lines().collect();

    for opt in ["-p", "--pkgs"] {
        // invalid
        cmd("pkgcruft replay")
            .args([opt, "%invalid"])
            .arg(file.path())
            .assert()
            .stdout("")
            .stderr(contains("invalid dep restriction: %invalid"))
            .failure()
            .code(2);

        for (target, expected) in [
            ("sys-fs/*", &expected[0..=0]),
            ("*m*", &expected[0..]),
            ("mutter", &expected[1..=2]),
            ("mutter-45.1", &expected[1..=1]),
            ("*", &expected),
        ] {
            let output = cmd("pkgcruft replay -R json -")
                .args([opt, target])
                .write_stdin(reports)
                .output()
                .unwrap()
                .stdout;
            let output = String::from_utf8(output).unwrap();
            let output: Vec<_> = output.lines().collect();
            assert_eq!(&output, expected);
        }
    }
}

#[test]
fn sort() {
    // serialized reports in reversed sorting order
    let reports = indoc::indoc! {r#"
        {"kind":"UnstableOnly","scope":{"Package":"cat/pkg"},"message":"x86"}
        {"kind":"WhitespaceInvalid","scope":{"Version":["cat/pkg-2",{"line":16,"column":0}]},"message":"missing ending newline"}
        {"kind":"WhitespaceInvalid","scope":{"Version":["cat/pkg-2",{"line":4,"column":17}]},"message":"character '\\u{2002}'"}
        {"kind":"WhitespaceInvalid","scope":{"Version":["cat/pkg-2",{"line":4,"column":5}]},"message":"character '\\u{2001}'"}
        {"kind":"WhitespaceUnneeded","scope":{"Version":["cat/pkg-2",{"line":3,"column":0}]},"message":"empty line"}
        {"kind":"DependencyDeprecated","scope":{"Version":["cat/pkg-2",null]},"message":"BDEPEND: pkg/deprecated"}
        {"kind":"DependencyDeprecated","scope":{"Version":["cat/pkg-1",null]},"message":"BDEPEND: pkg/deprecated"}
    "#};
    let mut expected: Vec<_> = reports.lines().collect();
    expected.reverse();

    let output = cmd("pkgcruft replay -R json --sort -")
        .write_stdin(reports)
        .output()
        .unwrap()
        .stdout;
    let output = String::from_utf8(output).unwrap();
    let output: Vec<_> = output.lines().collect();
    assert_eq!(&output, &expected);
}

#[test]
fn reporter() {
    let file = qa_primary_file();
    for opt in ["-R", "--reporter"] {
        // invalid
        cmd("pkgcruft replay")
            .args([opt, "invalid"])
            .arg(file.path())
            .assert()
            .stdout("")
            .stderr(contains("--reporter"))
            .failure()
            .code(2);

        for reporter in ["simple", "fancy", "json"] {
            cmd("pkgcruft replay")
                .args([opt, reporter])
                .arg(file.path())
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }

        // missing format string
        cmd("pkgcruft replay")
            .args([opt, "format"])
            .arg(file.path())
            .assert()
            .stdout("")
            .stderr(contains("--format"))
            .failure()
            .code(2);

        // invalid format string
        cmd("pkgcruft replay")
            .args([opt, "format"])
            .args(["--format", "{format}"])
            .arg(file.path())
            .assert()
            .stdout("")
            .stderr(contains("invalid output format"))
            .failure()
            .code(2);

        // valid format string
        cmd("pkgcruft replay")
            .args([opt, "format"])
            .args(["--format", "{name}"])
            .arg(file.path())
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}
