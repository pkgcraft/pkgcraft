use std::fs;
use std::io::Write;
use std::sync::LazyLock;

use itertools::Itertools;
use pkgcraft::repo::Repository;
use pkgcraft::test::cmd;
use pkgcruft::test::*;
use predicates::prelude::*;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::NamedTempFile;

/// Temporary file of all serialized reports from the primary QA test repo.
pub(crate) static QA_PRIMARY_FILE: LazyLock<NamedTempFile> = LazyLock::new(|| {
    let mut file = NamedTempFile::new().unwrap();
    let output = cmd("pkgcruft scan -R json")
        .arg(qa_repo("qa-primary"))
        .output()
        .unwrap()
        .stdout;
    file.write_all(&output).unwrap();
    file
});

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
    // valid
    cmd("pkgcruft replay -")
        .write_stdin(fs::read_to_string(QA_PRIMARY_FILE.path()).unwrap())
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
    cmd("pkgcruft replay")
        .arg(QA_PRIMARY_FILE.path())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

#[test]
fn checks() {
    let repo = qa_repo("qa-primary").path();
    let single_expected = glob_reports!("{repo}/Dependency/**/reports.json");
    let multiple_expected = glob_reports!(
        "{repo}/Dependency/**/reports.json",
        "{repo}/EapiStatus/**/reports.json",
        "{repo}/Keywords/**/reports.json",
    );
    let data = multiple_expected.iter().map(|x| x.to_json()).join("\n");

    for opt in ["-c", "--checks"] {
        // invalid
        cmd("pkgcruft replay")
            .args([opt, "invalid"])
            .arg(QA_PRIMARY_FILE.path())
            .assert()
            .stdout("")
            .stderr(contains("--checks"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft replay -R json -")
            .args([opt, "Dependency"])
            .write_stdin(data.as_str())
            .to_reports()
            .unwrap();
        assert_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft replay -R json -")
            .args([opt, "Dependency,EapiStatus,Keywords"])
            .write_stdin(data.as_str())
            .to_reports()
            .unwrap();
        assert_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn levels() {
    let repo = qa_repo("qa-primary").path();
    let single_expected = glob_reports!("{repo}/EapiStatus/EapiDeprecated/reports.json");
    let multiple_expected = glob_reports!("{repo}/EapiStatus/**/reports.json");
    let data = multiple_expected.iter().map(|x| x.to_json()).join("\n");

    for opt in ["-l", "--levels"] {
        // invalid
        cmd("pkgcruft replay")
            .args([opt, "invalid"])
            .arg(QA_PRIMARY_FILE.path())
            .assert()
            .stdout("")
            .stderr(contains("--levels"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft replay -R json -")
            .args([opt, "warning"])
            .write_stdin(data.as_str())
            .to_reports()
            .unwrap();
        assert_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft replay -R json -")
            .args([opt, "warning,error"])
            .write_stdin(data.as_str())
            .to_reports()
            .unwrap();
        assert_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn reports() {
    let repo = qa_repo("qa-primary").path();
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
            .arg(QA_PRIMARY_FILE.path())
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
    let repo = qa_repo("qa-primary").path();
    let single_expected = glob_reports!("{repo}/Dependency/DependencyDeprecated/reports.json");
    let multiple_expected = glob_reports!(
        "{repo}/Dependency/DependencyDeprecated/reports.json",
        "{repo}/UnstableOnly/UnstableOnly/reports.json",
    );
    let data = multiple_expected.iter().map(|x| x.to_json()).join("\n");

    for opt in ["-s", "--scopes"] {
        // invalid
        cmd("pkgcruft replay")
            .args([opt, "invalid"])
            .arg(QA_PRIMARY_FILE.path())
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
fn sources() {
    let repo = qa_repo("qa-primary").path();
    let expected = glob_reports!(
        "{repo}/Dependency/DependencyDeprecated/reports.json",
        "{repo}/UnstableOnly/UnstableOnly/reports.json",
    );
    let data = expected.iter().map(|x| x.to_json()).join("\n");

    for opt in ["-S", "--sources"] {
        // invalid
        cmd("pkgcruft replay")
            .args([opt, "invalid"])
            .arg(QA_PRIMARY_FILE.path())
            .assert()
            .stdout("")
            .stderr(contains("--sources"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft replay -R json -")
            .args([opt, "ebuild-pkg"])
            .write_stdin(data.as_str())
            .to_reports()
            .unwrap();
        assert_eq!(&expected, &reports);

        // TODO: add test for multiple args once issue #178 is fixed
    }
}

#[test]
fn pkgs() {
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
            .arg(QA_PRIMARY_FILE.path())
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
    for opt in ["-R", "--reporter"] {
        // invalid
        cmd("pkgcruft replay")
            .args([opt, "invalid"])
            .arg(QA_PRIMARY_FILE.path())
            .assert()
            .stdout("")
            .stderr(contains("--reporter"))
            .failure()
            .code(2);

        for reporter in ["simple", "fancy", "json"] {
            cmd("pkgcruft replay")
                .args([opt, reporter])
                .arg(QA_PRIMARY_FILE.path())
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }

        // missing format string
        cmd("pkgcruft replay")
            .args([opt, "format"])
            .arg(QA_PRIMARY_FILE.path())
            .assert()
            .stdout("")
            .stderr(contains("--format"))
            .failure()
            .code(2);

        // invalid format string
        cmd("pkgcruft replay")
            .args([opt, "format"])
            .args(["--format", "{format}"])
            .arg(QA_PRIMARY_FILE.path())
            .assert()
            .stdout("")
            .stderr(contains("invalid output format"))
            .failure()
            .code(2);

        // valid format string
        cmd("pkgcruft replay")
            .args([opt, "format"])
            .args(["--format", "{package}"])
            .arg(QA_PRIMARY_FILE.path())
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}
