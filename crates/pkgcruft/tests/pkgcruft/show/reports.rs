use itertools::Itertools;
use pkgcraft::test::cmd;
use pkgcraft::restrict::Scope;
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
