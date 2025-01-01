use std::env;

use itertools::Itertools;
use pkgcraft::restrict::Scope;
use pkgcraft::test::{cmd, test_data};
use pkgcruft::check::CheckKind;
use pkgcruft::report::{ReportKind, ReportLevel};
use predicates::prelude::*;
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
        for check in CheckKind::iter() {
            cmd(format!("pkgcruft show reports {opt} @{check}"))
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }

        for level in ReportLevel::iter() {
            cmd(format!("pkgcruft show reports {opt} @{level}"))
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }

        for report in ReportKind::iter() {
            cmd(format!("pkgcruft show reports {opt} {report}"))
                .assert()
                .stdout(format!("{report}\n"))
                .stderr("")
                .success();
        }

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

    cmd("pkgcruft show reports --repo")
        .arg(repo)
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
