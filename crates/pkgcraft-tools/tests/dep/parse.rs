use itertools::Itertools;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;

#[test]
fn valid() {
    let data = test_data();
    let deps = data.dep_toml.valid.iter().map(|e| &e.dep).join("\n");
    cmd("pk dep parse -")
        .write_stdin(deps)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn invalid() {
    let data = test_data();
    let deps = data.dep_toml.invalid.iter().join("\n");
    cmd("pk dep parse -")
        .write_stdin(deps)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();
}

#[test]
fn eapi() {
    // old and unsupported
    cmd("pk dep parse --eapi 0 cat/pkg")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();

    // invalid
    cmd("pk dep parse --eapi $0 cat/pkg")
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();

    // valid and suppored
    cmd("pk dep parse --eapi 8 cat/pkg[use(+)]")
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn format() {
    for opt in ["-f", "--format"] {
        for (args, expected) in [
            (["{BLOCKER}", "cat/pkg"], "<unset>"),
            (["{BLOCKER}", "!cat/pkg"], "!"),
            (["{BLOCKER}", "!!cat/pkg"], "!!"),
            (["{CATEGORY}", "cat/pkg"], "cat"),
            (["{P}", "cat/pkg"], "<unset>"),
            (["{P}", "=cat/pkg-1-r2"], "pkg-1"),
            (["{PF}", "cat/pkg"], "<unset>"),
            (["{PF}", "=cat/pkg-1-r2"], "pkg-1-r2"),
            (["{PN}", "=cat/pkg-1-r2"], "pkg"),
            (["{PR}", "cat/pkg"], "<unset>"),
            (["{PR}", "=cat/pkg-1"], "r0"),
            (["{PR}", "=cat/pkg-1-r2"], "r2"),
            (["{PV}", "cat/pkg"], "<unset>"),
            (["{PV}", "=cat/pkg-1-r2"], "1"),
            (["{PVR}", "cat/pkg"], "<unset>"),
            (["{PVR}", "=cat/pkg-1-r2"], "1-r2"),
            (["{CPN}", "cat/pkg"], "cat/pkg"),
            (["{CPN}", "=cat/pkg-1-r2"], "cat/pkg"),
            (["{CPV}", "cat/pkg"], "<unset>"),
            (["{CPV}", "=cat/pkg-1-r2"], "cat/pkg-1-r2"),
            (["{OP}", "cat/pkg"], "<unset>"),
            (["{OP}", "=cat/pkg-1-r2"], "="),
            (["{SLOT}", "=cat/pkg-1-r2"], "<unset>"),
            (["{SLOT}", "=cat/pkg-1-r2:0"], "0"),
            (["{SUBSLOT}", "=cat/pkg-1-r2"], "<unset>"),
            (["{SUBSLOT}", "=cat/pkg-1-r2:0"], "<unset>"),
            (["{SUBSLOT}", "=cat/pkg-1-r2:0/3"], "3"),
            (["{SLOT_OP}", "cat/pkg"], "<unset>"),
            (["{SLOT_OP}", "=cat/pkg-1-r2:="], "="),
            (["{SLOT_OP}", "cat/pkg:0="], "="),
            (["{REPO}", "=cat/pkg-1-r2"], "<unset>"),
            (["{REPO}", "=cat/pkg-1-r2::repo"], "repo"),
            (["{USE}", "cat/pkg"], "<unset>"),
            (["{USE}", "cat/pkg[u1,u2]"], "u1,u2"),
            (["{USE}", "=cat/pkg-1-r2[u1,u2]"], "u1,u2"),
            (["{DEP}", "cat/pkg"], "cat/pkg"),
            (["{DEP}", "=cat/pkg-1-r2"], "=cat/pkg-1-r2"),
        ] {
            cmd("pk dep parse")
                .arg(opt)
                .args(args)
                .assert()
                .stdout(predicate::str::diff(expected).trim())
                .stderr("")
                .success();
        }
    }
}
