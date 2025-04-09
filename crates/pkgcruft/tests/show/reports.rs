use std::env;

use itertools::Itertools;
use pkgcraft::restrict::Scope;
use pkgcraft::test::{cmd, test_data};
use pkgcruft::check::{Check, Context};
use pkgcruft::report::{ReportKind, ReportLevel};
use predicates::prelude::*;
use predicates::str::contains;
use strum::IntoEnumIterator;

#[test]
fn all() {
    cmd("pkgcruft show reports")
        .assert()
        .stdout(indoc::formatdoc! {"
            {}
        ", ReportKind::iter().join("\n")})
        .stderr("")
        .success();
}

#[test]
fn sets() {
    for opt in ["-r", "--reports"] {
        // invalid
        cmd("pkgcruft show reports")
            .args([opt, "invalid"])
            .assert()
            .stdout("")
            .stderr(contains("invalid report: invalid"))
            .failure()
            .code(2);

        // all supported
        cmd(format!("pkgcruft show reports {opt} @all"))
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();

        // check
        let check = Check::iter().next().unwrap();
        cmd(format!("pkgcruft show reports {opt} @{check}"))
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();

        // context
        let context = Context::iter().next().unwrap();
        cmd(format!("pkgcruft show reports {opt} @{context}"))
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();

        // report level
        let level = ReportLevel::iter().next().unwrap();
        cmd(format!("pkgcruft show reports {opt} @{level}"))
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();

        // report
        let report = ReportKind::iter().next().unwrap();
        cmd(format!("pkgcruft show reports {opt} {report}"))
            .assert()
            .stdout(format!("{report}\n"))
            .stderr("")
            .success();

        // report scope
        let scope = Scope::iter().next().unwrap();
        cmd(format!("pkgcruft show reports {opt} @{scope}"))
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}

#[test]
fn repo() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();

    // nonexistent
    cmd("pkgcruft show reports --repo nonexistent")
        .assert()
        .stdout("")
        .stderr(contains("nonexistent repo: nonexistent"))
        .failure()
        .code(2);

    // specific path
    cmd("pkgcruft show reports")
        .args(["--repo", repo.as_ref()])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // specific path with check set
    cmd("pkgcruft show reports")
        .args(["--repo", repo.as_ref()])
        .args(["-r", "@Dependency"])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // current working directory
    env::set_current_dir(repo).unwrap();
    cmd("pkgcruft show reports --repo")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}
