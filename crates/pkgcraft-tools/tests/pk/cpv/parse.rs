use itertools::Itertools;
use pkgcraft::test::cmd;
use predicates::prelude::*;

#[test]
fn valid() {
    let input = ["cat/pkg-1", "cat/pkg-2"].iter().join("\n");
    cmd("pk cpv parse -")
        .write_stdin(input)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn invalid() {
    let input = ["cat/pkg-1", "cat/pkg"].iter().join("\n");
    cmd("pk cpv parse -")
        .write_stdin(input)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure();
}

#[test]
fn format() {
    for opt in ["-f", "--format"] {
        for (args, expected) in [
            (["{CATEGORY}", "cat/pkg-1"], "cat"),
            (["{P}", "cat/pkg-1"], "pkg-1"),
            (["{P}", "cat/pkg-1-r2"], "pkg-1"),
            (["{PF}", "cat/pkg-1"], "pkg-1"),
            (["{PF}", "cat/pkg-1-r2"], "pkg-1-r2"),
            (["{PN}", "cat/pkg-1-r2"], "pkg"),
            (["{PR}", "cat/pkg-1"], "r0"),
            (["{PR}", "cat/pkg-1-r2"], "r2"),
            (["{PV}", "cat/pkg-1"], "1"),
            (["{PV}", "cat/pkg-1-r2"], "1"),
            (["{PVR}", "cat/pkg-1"], "1"),
            (["{PVR}", "cat/pkg-1-r2"], "1-r2"),
            (["{CPN}", "cat/pkg-1"], "cat/pkg"),
            (["{CPV}", "cat/pkg-1"], "cat/pkg-1"),
            (["{CPV}", "cat/pkg-1-r2"], "cat/pkg-1-r2"),
        ] {
            cmd("pk cpv parse")
                .arg(opt)
                .args(args)
                .assert()
                .stdout(predicate::str::diff(expected).trim())
                .stderr("")
                .success();
        }
    }
}
