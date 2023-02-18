use pkgcraft::test::{cmd, DepToml};
use predicates::prelude::*;

#[test]
fn args() {
    // intersect
    cmd("pk dep intersect")
        .args(["a/a", "a/a"])
        .assert()
        .success();
    cmd("pk dep intersect")
        .args(["cat/pkg[u]", "cat/pkg[u,u]"])
        .assert()
        .success();

    // non-intersect
    cmd("pk dep intersect")
        .args(["a/a", "a/b"])
        .assert()
        .code(1);

    // errors return exit code 2 and output message to stderr
    for args in [vec!["a/b"], vec!["a/b", "a/b/c"]] {
        cmd("pk dep intersect")
            .args(&args)
            .assert()
            .code(2)
            .stderr(predicate::str::is_empty().not());
    }
}

#[test]
fn toml() {
    let data = DepToml::load().unwrap();
    for d in data.intersects {
        let status = cmd("pk dep intersect").args(&d.vals).assert();
        if d.status {
            status.success();
        } else {
            status.failure();
        }
    }
}
