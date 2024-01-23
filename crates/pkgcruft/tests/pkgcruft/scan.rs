use std::env;

use pkgcraft::repo::Repository;
use pkgcraft::test::{cmd, TEST_DATA};
use pkgcraft::utils::current_dir;
use pkgcruft::report::Report;
use pkgcruft::test::glob_reports;
use predicates::str::contains;

#[test]
fn invalid_dep_restricts() {
    for s in ["^pkg", "cat&pkg"] {
        cmd("pkgcruft scan")
            .arg(s)
            .assert()
            .stdout("")
            .stderr(contains(format!("invalid dep restriction: {s}")))
            .failure()
            .code(2);
    }
}

#[test]
fn stdin_targets() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    for arg in ["DroppedKeywords", "DroppedKeywords/DroppedKeywords"] {
        cmd("pkgcruft scan -R simple -")
            .args(["--repo", repo.path().as_ref()])
            .write_stdin(format!("{arg}\n"))
            .assert()
            .stdout(contains("DroppedKeywords: arm64"))
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
    let expected: Vec<_> = glob_reports(format!("{repo_path}/**/reports.json")).collect();
    let output = cmd("pkgcruft scan -j1 -R json").output().unwrap().stdout;
    let data = String::from_utf8(output).unwrap();
    let reports: Vec<_> = data
        .lines()
        .map(|s| Report::from_json(s).unwrap())
        .collect();
    assert_eq!(&expected, &reports);

    // category dir
    env::set_current_dir(repo.path().join("Dependency")).unwrap();
    let expected: Vec<_> =
        glob_reports(format!("{repo_path}/Dependency/**/reports.json")).collect();
    let output = cmd("pkgcruft scan -j1 -R json").output().unwrap().stdout;
    let data = String::from_utf8(output).unwrap();
    let reports: Vec<_> = data
        .lines()
        .map(|s| Report::from_json(s).unwrap())
        .collect();
    assert_eq!(&expected, &reports);

    // package dir
    env::set_current_dir(repo.path().join("Dependency/DeprecatedDependency")).unwrap();
    let expected: Vec<_> =
        glob_reports(format!("{repo_path}/Dependency/DeprecatedDependency/reports.json")).collect();
    let output = cmd("pkgcruft scan -j1 -R json").output().unwrap().stdout;
    let data = String::from_utf8(output).unwrap();
    let reports: Vec<_> = data
        .lines()
        .map(|s| Report::from_json(s).unwrap())
        .collect();
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
    let expected: Vec<_> = glob_reports(format!("{repo_path}/**/reports.json")).collect();
    let output = cmd("pkgcruft scan -j1 -R json")
        .arg(repo.path())
        .output()
        .unwrap()
        .stdout;
    let data = String::from_utf8(output).unwrap();
    let reports: Vec<_> = data
        .lines()
        .map(|s| Report::from_json(s).unwrap())
        .collect();
    assert_eq!(&expected, &reports);

    // category dir
    let expected: Vec<_> =
        glob_reports(format!("{repo_path}/Dependency/**/reports.json")).collect();
    let output = cmd("pkgcruft scan -j1 -R json")
        .arg(repo.path().join("Dependency"))
        .output()
        .unwrap()
        .stdout;
    let data = String::from_utf8(output).unwrap();
    let reports: Vec<_> = data
        .lines()
        .map(|s| Report::from_json(s).unwrap())
        .collect();
    assert_eq!(&expected, &reports);

    // package dir
    let expected: Vec<_> =
        glob_reports(format!("{repo_path}/Dependency/DeprecatedDependency/reports.json")).collect();
    let output = cmd("pkgcruft scan -j1 -R json")
        .arg(repo.path().join("Dependency/DeprecatedDependency"))
        .output()
        .unwrap()
        .stdout;
    let data = String::from_utf8(output).unwrap();
    let reports: Vec<_> = data
        .lines()
        .map(|s| Report::from_json(s).unwrap())
        .collect();
    assert_eq!(&expected, &reports);
}
