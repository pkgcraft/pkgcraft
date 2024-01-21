use std::env;

use pkgcraft::repo::Repository;
use pkgcraft::test::{cmd, TEST_DATA};
use pkgcraft::utils::current_dir;
use predicates::prelude::*;
use predicates::str::contains;

#[test]
fn invalid_cwd() {
    let path = current_dir().unwrap();
    cmd("pkgcruft scan")
        .assert()
        .stdout("")
        .stderr(contains(format!("invalid ebuild repo: {path}")))
        .failure()
        .code(2);
}

#[test]
fn nonexistent_path_target() {
    cmd("pkgcruft scan path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("invalid path target"))
        .failure()
        .code(2);
}

#[test]
fn invalid_path_target() {
    cmd("pkgcruft scan /")
        .assert()
        .stdout("")
        .stderr(contains("invalid ebuild repo: /"))
        .failure()
        .code(2);
}

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
    env::set_current_dir(repo.path()).unwrap();
    for arg in ["DroppedKeywords", "DroppedKeywords/DroppedKeywords"] {
        cmd("pkgcruft scan -R simple -")
            .write_stdin(format!("{arg}\n"))
            .assert()
            .stdout(contains("DroppedKeywords: arm64"))
            .stderr("")
            .success();
    }
}

#[test]
fn repo_path_target() {
    let repo = TEST_DATA.ebuild_repo("qa-primary").unwrap();
    cmd("pkgcruft scan")
        .arg(repo.path())
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
