use std::env;

use itertools::Itertools;
use pkgcraft::restrict::Scope;
use pkgcraft::test::{cmd, test_data};
use pkgcruft::check::CheckKind;
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
fn aliases() {
    for opt in ["-r", "--reports"] {
        // invalid
        cmd("pkgcruft show reports")
            .args([opt, "invalid"])
            .assert()
            .stdout("")
            .stderr(contains("invalid report: invalid"))
            .failure()
            .code(2);

        // checks
        for check in CheckKind::iter() {
            cmd(format!("pkgcruft show reports {opt} @{check}"))
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }

        // report levels
        for level in ReportLevel::iter() {
            cmd(format!("pkgcruft show reports {opt} @{level}"))
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }

        // reports
        for report in ReportKind::iter() {
            cmd(format!("pkgcruft show reports {opt} {report}"))
                .assert()
                .stdout(format!("{report}\n"))
                .stderr("")
                .success();
        }

        // report scopes
        for scope in Scope::iter() {
            cmd(format!("pkgcruft show reports {opt} @{scope}"))
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }
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
        .stderr(contains("unknown repo: nonexistent"))
        .failure()
        .code(2);

    // specific path
    cmd("pkgcruft show reports")
        .args(["--repo", repo.as_ref()])
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();

    // specific path with report alias
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
