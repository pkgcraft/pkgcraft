use predicates::prelude::*;
use predicates::str::contains;

use crate::cmd;

#[test]
fn args() {
    // intersect
    for args in [
        ["a/b-1", "a/b-1"],
        ["cat/pkg-1", "cat/pkg"],
        ["cat/pkg-1", "=cat/pkg-1*"],
        ["cat/pkg-1", "~cat/pkg-1"],
        ["cat/pkg-1", "cat/pkg[u]"],
    ] {
        cmd("pk cpv intersect").args(args).assert().success();
    }

    // non-intersect
    for args in [
        ["a/a-1", "b/a-1"],
        ["a/a-1", "a/b-1"],
        ["a/a-1", "a/a-2"],
        ["cat/pkg-1", "=cat/pkg-1.1*"],
        ["cat/pkg-2", "<cat/pkg-2"],
    ] {
        cmd("pk cpv intersect")
            .args(args)
            .assert()
            .failure()
            .code(1);
    }

    // Dep objects can only be in the second position
    cmd("pk cpv intersect")
        .args(["=cat/pkg-1", "cat/pkg-1"])
        .assert()
        .stdout("")
        .stderr(contains("invalid cpv: =cat/pkg-1"))
        .failure()
        .code(2);

    // errors return exit code 2 and output message to stderr
    for args in [vec!["a/b-1"], vec!["a/b-1", "a/b/c"]] {
        cmd("pk cpv intersect")
            .args(&args)
            .assert()
            .stderr(predicate::str::is_empty().not())
            .failure()
            .code(2);
    }
}
