use itertools::Itertools;
use pkgcraft::test::cmd;
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
fn levels() {
    for opt in ["-r", "--reports"] {
        for level in ReportLevel::iter() {
            cmd(format!("pkgcruft show reports {opt} @{level}"))
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }
    }
}
