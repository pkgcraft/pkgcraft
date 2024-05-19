use std::env;

use assert_cmd::Command;
use pkgcraft::repo::Repository;
use pkgcraft::test::{cmd, TEST_DATA};
use pkgcraft::utils::current_dir;
use pkgcruft::report::Report;
use pkgcruft::test::glob_reports;
use predicates::prelude::*;
use predicates::str::contains;

trait IntoReports {
    fn to_reports(&mut self) -> Vec<Report>;
}

impl IntoReports for Command {
    fn to_reports(&mut self) -> Vec<Report> {
        let output = self.output().unwrap().stdout;
        let data = String::from_utf8(output).unwrap();
        data.lines()
            .map(|s| Report::from_json(s).unwrap())
            .collect()
    }
}

#[test]
fn stdin_targets() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    for arg in ["DroppedKeywords", "DroppedKeywords/DroppedKeywords"] {
        cmd("pkgcruft scan -R simple -")
            .args(["--repo", repo.path().as_str()])
            .write_stdin(format!("{arg}\n"))
            .assert()
            .stdout(contains("DroppedKeywords: x86"))
            .stderr("")
            .success();
    }
}

#[test]
fn dep_restrict_targets() {
    // invalid
    for s in ["^pkg", "cat&pkg"] {
        cmd("pkgcruft scan")
            .arg(s)
            .assert()
            .stdout("")
            .stderr(contains(format!("invalid dep restriction: {s}")))
            .failure()
            .code(2);
    }

    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let repo_path = repo.path();

    // valid
    for s in ["DroppedKeywords/*", "DroppedKeywords"] {
        cmd("pkgcruft scan -R simple")
            .args(["--repo", repo_path.as_str()])
            .arg(s)
            .assert()
            .stdout(contains("DroppedKeywords: x86"))
            .stderr("")
            .success();
    }
}

#[test]
fn current_dir_targets() {
    // invalid
    let path = current_dir().unwrap();
    cmd("pkgcruft scan")
        .assert()
        .stdout("")
        .stderr(contains(format!("invalid ebuild repo: {path}")))
        .failure()
        .code(2);

    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let repo_path = repo.path();

    // repo dir
    env::set_current_dir(repo.path()).unwrap();
    let expected = glob_reports!("{repo_path}/**/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json").to_reports();
    assert_eq!(&expected, &reports);

    // category dir
    env::set_current_dir(repo.path().join("Dependency")).unwrap();
    let expected = glob_reports!("{repo_path}/Dependency/**/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json").to_reports();
    assert_eq!(&expected, &reports);

    // package dir
    env::set_current_dir(repo.path().join("Dependency/DeprecatedDependency")).unwrap();
    let expected = glob_reports!("{repo_path}/Dependency/DeprecatedDependency/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json").to_reports();
    assert_eq!(&expected, &reports);
}

#[test]
fn path_targets() {
    // nonexistent
    cmd("pkgcruft scan path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("invalid path target"))
        .failure()
        .code(2);

    // invalid
    cmd("pkgcruft scan /")
        .assert()
        .stdout("")
        .stderr(contains("invalid ebuild repo: /"))
        .failure()
        .code(2);

    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let repo_path = repo.path();

    // repo dir
    let expected = glob_reports!("{repo_path}/**/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json")
        .arg(repo.path())
        .to_reports();
    assert_eq!(&expected, &reports);

    // category dir
    let expected = glob_reports!("{repo_path}/Dependency/**/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json")
        .arg(repo.path().join("Dependency"))
        .to_reports();
    assert_eq!(&expected, &reports);

    // package dir
    let expected = glob_reports!("{repo_path}/Dependency/DeprecatedDependency/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json")
        .arg(repo.path().join("Dependency/DeprecatedDependency"))
        .to_reports();
    assert_eq!(&expected, &reports);
}

#[test]
fn repo() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let repo_path = repo.path();

    env::set_current_dir(repo.path()).unwrap();
    for path in [".", "./", repo_path.as_str()] {
        // implicit target
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args(["--repo", path])
            .to_reports();
        let expected = glob_reports!("{repo_path}/**/reports.json");
        assert_eq!(&expected, &reports);

        // category target
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args(["--repo", path])
            .arg("Dependency/*")
            .to_reports();
        let expected = glob_reports!("{repo_path}/Dependency/**/reports.json");
        assert_eq!(&expected, &reports);

        // pkg target
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args(["--repo", path])
            .arg("DeprecatedDependency")
            .to_reports();
        let expected = glob_reports!("{repo_path}/Dependency/DeprecatedDependency/reports.json");
        assert_eq!(&expected, &reports);
    }

    // implicit target set to all packages when targeting a repo
    let qa_overlay = TEST_DATA.ebuild_repo("qa-secondary").unwrap();
    env::set_current_dir(qa_overlay.path()).unwrap();
    let reports = cmd("pkgcruft scan -j1 -R json")
        .args(["--repo", repo_path.as_str()])
        .to_reports();
    let expected = glob_reports!("{repo_path}/**/reports.json");
    assert_eq!(&expected, &reports);
}

#[test]
fn reporter() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    env::set_current_dir(repo.path()).unwrap();

    for opt in ["-R", "--reporter"] {
        // invalid
        cmd("pkgcruft scan -j1")
            .args([opt, "invalid"])
            .assert()
            .stdout("")
            .stderr(predicate::str::is_empty().not())
            .failure()
            .code(2);

        for reporter in ["simple", "fancy", "json"] {
            cmd("pkgcruft scan -j1")
                .args([opt, reporter])
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }

        // missing format string
        cmd("pkgcruft scan -j1")
            .args([opt, "format"])
            .assert()
            .stdout("")
            .stderr(contains("--format"))
            .failure()
            .code(2);

        cmd("pkgcruft scan -j1")
            .args([opt, "format"])
            .args(["--format", "{package}"])
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}

#[test]
fn checks() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let repo_path = repo.path();
    let expected = glob_reports!("{repo_path}/Dependency/**/reports.json");

    for opt in ["-c", "--checks"] {
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "Dependency"])
            .arg(repo.path())
            .to_reports();
        assert_eq!(&expected, &reports);
    }
}

#[test]
fn reports() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let repo_path = repo.path();
    let expected = glob_reports!("{repo_path}/Dependency/DeprecatedDependency/reports.json");

    for opt in ["-r", "--reports"] {
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "DeprecatedDependency"])
            .arg(repo.path())
            .to_reports();
        assert_eq!(&expected, &reports);
    }
}
