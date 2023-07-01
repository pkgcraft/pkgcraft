use itertools::Itertools;
use pkgcraft::test::{cmd, TEST_DATA};
use predicates::prelude::*;

use crate::predicates::lines_contain;

#[test]
fn valid() {
    let deps = TEST_DATA.dep_toml.valid.iter().map(|e| &e.dep).join("\n");
    cmd("pk dep parse -")
        .write_stdin(deps)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn invalid() {
    let deps = TEST_DATA.dep_toml.invalid.iter().join("\n");
    cmd("pk dep parse -")
        .write_stdin(deps)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();
}

#[test]
fn eapi() {
    // use deps in EAPI >= 2
    cmd("pk dep parse --eapi 0 cat/pkg[use]")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();

    cmd("pk dep parse --eapi 2 cat/pkg[use]")
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn format() {
    for opt in ["-f", "--format"] {
        for (args, expected) in [
            (["{BLOCKER}", "!cat/pkg"], "!"),
            (["{BLOCKER}", "!!cat/pkg"], "!!"),
            (["{BLOCKER}", "cat/pkg"], "<unset>"),
            (["{CATEGORY}", "cat/pkg"], "cat"),
            (["{P}", "=cat/pkg-1-r2"], "pkg-1"),
            (["{PF}", "=cat/pkg-1-r2"], "pkg-1-r2"),
            (["{PN}", "=cat/pkg-1-r2"], "pkg"),
            (["{PR}", "=cat/pkg-1-r2"], "2"),
            (["{PV}", "=cat/pkg-1-r2"], "1"),
            (["{PVR}", "=cat/pkg-1-r2"], "1-r2"),
            (["{CPN}", "=cat/pkg-1-r2"], "cat/pkg"),
            (["{CPV}", "=cat/pkg-1-r2"], "cat/pkg-1-r2"),
            (["{OP}", "=cat/pkg-1-r2"], "="),
            (["{SLOT}", "=cat/pkg-1-r2"], "<unset>"),
            (["{SLOT}", "=cat/pkg-1-r2:0"], "0"),
            (["{SUBSLOT}", "=cat/pkg-1-r2"], "<unset>"),
            (["{SUBSLOT}", "=cat/pkg-1-r2:0/3"], "3"),
            (["{SLOT_OP}", "=cat/pkg-1-r2"], "<unset>"),
            (["{SLOT_OP}", "=cat/pkg-1-r2:="], "="),
            (["{REPO}", "=cat/pkg-1-r2"], "<unset>"),
            (["{REPO}", "=cat/pkg-1-r2::repo"], "repo"),
            (["{DEP}", "=cat/pkg-1-r2"], "=cat/pkg-1-r2"),
        ] {
            cmd("pk dep parse")
                .arg(opt)
                .args(args)
                .assert()
                .stdout(lines_contain([expected]))
                .stderr("")
                .success();
        }
    }
}
