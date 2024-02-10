use pkgcraft::test::cmd;
use pkgcruft::report::ReportLevel;
use predicates::prelude::*;
use strum::IntoEnumIterator;

#[test]
fn output() {
    cmd("pkgcruft show reports")
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr("")
        .success();
}

// TODO: assert output exists when reports of every level exist
#[test]
fn levels() {
    for opt in ["-l", "--levels"] {
        for level in ReportLevel::iter() {
            cmd("pkgcruft show reports")
                .args([opt, level.as_ref()])
                .assert()
                .stderr("")
                .success();
        }
    }
}
