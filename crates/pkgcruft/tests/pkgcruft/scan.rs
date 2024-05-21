use std::env;

use pkgcraft::repo::Repository;
use pkgcraft::test::{cmd, TEST_DATA};
use pkgcraft::utils::current_dir;
use pkgcruft::test::glob_reports;
use predicates::prelude::*;
use predicates::str::contains;
use pretty_assertions::assert_eq;

use crate::ToReports;

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

    // single
    for s in ["DroppedKeywords/*", "DroppedKeywords"] {
        cmd("pkgcruft scan -R simple")
            .args(["--repo", repo_path.as_str()])
            .arg(s)
            .assert()
            .stdout(contains("DroppedKeywords: x86"))
            .stderr("")
            .success();
    }

    // multiple matching restricts output the same reports
    let reports = indoc::indoc! {r#"
        DroppedKeywords/DroppedKeywords-2: DroppedKeywords: x86
        DroppedKeywords/DroppedKeywords-2: DroppedKeywords: x86
    "#};
    let expected: Vec<_> = reports.lines().collect();
    let output = cmd("pkgcruft scan -R simple")
        .args(["--repo", repo_path.as_str()])
        .args(["DroppedKeywords/*", "DroppedKeywords"])
        .output()
        .unwrap()
        .stdout;
    let data = String::from_utf8(output).unwrap();
    let data: Vec<_> = data.lines().collect();
    assert_eq!(&expected, &data);

    // nonexistent
    cmd("pkgcruft scan -R simple")
        .args(["--repo", repo_path.as_str()])
        .arg("nonexistent/pkg")
        .assert()
        .stdout("")
        .stderr("")
        .success();
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

    let primary = TEST_DATA.ebuild_repo("qa-primary").unwrap().path();
    let secondary = TEST_DATA.ebuild_repo("qa-secondary").unwrap().path();

    // repo dir
    let expected = glob_reports!("{primary}/**/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json").arg(primary).to_reports();
    assert_eq!(&expected, &reports);

    // overlay dir
    let expected = glob_reports!("{secondary}/**/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json").arg(secondary).to_reports();
    assert_eq!(&expected, &reports);

    // category dir
    let expected = glob_reports!("{primary}/Dependency/**/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json")
        .arg(primary.join("Dependency"))
        .to_reports();
    assert_eq!(&expected, &reports);

    // package dir
    let expected = glob_reports!("{primary}/Dependency/DeprecatedDependency/reports.json");
    let reports = cmd("pkgcruft scan -j1 -R json")
        .arg(primary.join("Dependency/DeprecatedDependency"))
        .to_reports();
    assert_eq!(&expected, &reports);

    // multiple absolute paths in the same repo
    let expected =
        glob_reports!("{primary}/Dependency/**/reports.json", "{primary}/Eapi/**/reports.json",);

    let reports = cmd("pkgcruft scan -j1 -R json")
        .arg(primary.join("Dependency"))
        .arg(primary.join("Eapi"))
        .to_reports();
    assert_eq!(&expected, &reports);

    // multiple relative paths in the same repo
    env::set_current_dir(primary).unwrap();
    let reports = cmd("pkgcruft scan -j1 -R json")
        .arg("Dependency")
        .arg("Eapi")
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
            .stderr(contains("--reporter"))
            .failure()
            .code(2);

        // valid
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

        // invalid format string
        cmd("pkgcruft scan -j1")
            .args([opt, "format"])
            .args(["--format", "{format}"])
            .assert()
            .stdout("")
            .stderr(contains("invalid output format"))
            .failure()
            .code(2);

        // valid format string
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
    let single_expected = glob_reports!("{repo_path}/Dependency/**/reports.json");
    let multiple_expected = glob_reports!(
        "{repo_path}/Dependency/**/reports.json",
        "{repo_path}/Eapi/**/reports.json",
    );

    for opt in ["-c", "--checks"] {
        // invalid
        cmd("pkgcruft scan -j1 -R json")
            .args([opt, "invalid"])
            .arg(repo.path())
            .assert()
            .stdout("")
            .stderr(contains("--checks"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "Dependency"])
            .arg(repo.path())
            .to_reports();
        assert_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "Dependency,Eapi"])
            .arg(repo.path())
            .to_reports();
        assert_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn levels() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let repo_path = repo.path();
    let single_expected = glob_reports!("{repo_path}/Eapi/EapiDeprecated/reports.json");
    let multiple_expected = glob_reports!("{repo_path}/Eapi/**/reports.json");

    for opt in ["-l", "--levels"] {
        // invalid
        cmd("pkgcruft scan -j1 -R json")
            .args([opt, "invalid"])
            .arg(repo.path())
            .assert()
            .stdout("")
            .stderr(contains("--levels"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "warning"])
            .arg(repo.path().join("Eapi"))
            .to_reports();
        assert_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "warning,error"])
            .arg(repo.path().join("Eapi"))
            .to_reports();
        assert_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn reports() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let repo_path = repo.path();
    let single_expected = glob_reports!("{repo_path}/Dependency/DeprecatedDependency/reports.json");
    let multiple_expected = glob_reports!(
        "{repo_path}/Dependency/DeprecatedDependency/reports.json",
        "{repo_path}/Eapi/EapiBanned/reports.json",
    );

    for opt in ["-r", "--reports"] {
        // invalid
        cmd("pkgcruft scan -j1 -R json")
            .args([opt, "invalid"])
            .arg(repo.path())
            .assert()
            .stdout("")
            .stderr(contains("--reports"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "DeprecatedDependency"])
            .arg(repo.path())
            .to_reports();
        assert_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "DeprecatedDependency,EapiBanned"])
            .arg(repo.path())
            .to_reports();
        assert_eq!(&multiple_expected, &reports);
    }
}

#[test]
fn scopes() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    let repo_path = repo.path();
    let single_expected = glob_reports!("{repo_path}/Dependency/DeprecatedDependency/reports.json");
    let multiple_expected = glob_reports!(
        "{repo_path}/Dependency/DeprecatedDependency/reports.json",
        "{repo_path}/UnstableOnly/UnstableOnly/reports.json",
    );

    for opt in ["-s", "--scopes"] {
        // invalid
        cmd("pkgcruft scan -j1 -R json")
            .args([opt, "invalid"])
            .arg(repo.path())
            .assert()
            .stdout("")
            .stderr(contains("--scopes"))
            .failure()
            .code(2);

        // single
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "version"])
            .arg(repo.path().join("Dependency/DeprecatedDependency"))
            .arg(repo.path().join("UnstableOnly/UnstableOnly"))
            .to_reports();
        assert_eq!(&single_expected, &reports);

        // multiple
        let reports = cmd("pkgcruft scan -j1 -R json")
            .args([opt, "version,package"])
            .arg(repo.path().join("Dependency/DeprecatedDependency"))
            .arg(repo.path().join("UnstableOnly/UnstableOnly"))
            .to_reports();
        assert_eq!(&multiple_expected, &reports);
    }
}
