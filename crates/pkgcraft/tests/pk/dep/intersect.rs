use pkgcraft::test::cmd;
use predicates::prelude::*;

#[test]
fn args() {
    // intersect
    for args in [
        ["a/a", "a/a"],
        ["cat/pkg[u]", "cat/pkg[u,u]"],
        ["cat/pkg-1", "~cat/pkg-1"],
        [">=cat/pkg-1", "cat/pkg-9999"],
    ] {
        cmd("pk dep intersect").args(args).assert().success();
    }

    // non-intersect
    for args in [
        ["a/a", "a/b"],
        ["=cat/pkg-1.1*", "cat/pkg-1"],
        ["cat/pkg-2", "<cat/pkg-2"],
        ["cat/pkg[-a]", "cat/pkg[a]"],
    ] {
        cmd("pk dep intersect")
            .args(args)
            .assert()
            .failure()
            .code(1);
    }

    // errors return exit code 2 and output message to stderr
    for args in [vec!["a/b"], vec!["a/b", "a/b/c"]] {
        cmd("pk dep intersect")
            .args(&args)
            .assert()
            .failure()
            .code(2)
            .stderr(predicate::str::is_empty().not());
    }
}
