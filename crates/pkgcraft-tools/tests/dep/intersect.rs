use predicates::prelude::*;
use predicates::str::contains;

use crate::cmd;

#[test]
fn args() {
    // intersect
    for args in [
        ["a/a", "a/a"],
        ["cat/pkg[u]", "cat/pkg[u,u]"],
        ["~cat/pkg-1", "cat/pkg-1"],
        [">=cat/pkg-1", "cat/pkg-9999"],
    ] {
        cmd("pk dep intersect").args(args).assert().success();
    }

    // non-intersect
    for args in [
        ["a/a", "a/b"],
        ["=cat/pkg-1.1*", "cat/pkg-1"],
        ["<cat/pkg-2", "cat/pkg-2"],
        ["cat/pkg[-a]", "cat/pkg[a]"],
    ] {
        cmd("pk dep intersect")
            .args(args)
            .assert()
            .failure()
            .code(1);
    }

    // Cpv objects can only be in the second position
    cmd("pk dep intersect")
        .args(["cat/pkg-1", "=cat/pkg-1"])
        .assert()
        .stdout("")
        .stderr(contains("invalid dep: cat/pkg-1"))
        .failure()
        .code(2);

    // errors return exit code 2 and output message to stderr
    for args in [vec!["a/b"], vec!["a/b", "a/b/c"]] {
        cmd("pk dep intersect")
            .args(&args)
            .assert()
            .stderr(predicate::str::is_empty().not())
            .failure()
            .code(2);
    }
}
