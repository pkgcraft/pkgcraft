use predicates::prelude::*;

use crate::cmd;

#[test]
fn args() {
    // intersect
    for args in [
        ["0", "0"],
        ["0", "00"],
        ["1.0", "1.00"],
        ["1", "~1"],
        [">=1", "9999"],
        ["1.2.3", "1.2.3-r0"],
    ] {
        cmd("pk version intersect").args(args).assert().success();
    }

    // non-intersect
    for args in [
        ["0", "1"],
        ["=1.1*", "1"],
        ["2", "<2"],
        ["=1.2a_alpha3_beta4-r5", "=1.2a_alpha3_beta4-r6"],
    ] {
        cmd("pk version intersect")
            .args(args)
            .assert()
            .failure()
            .code(1);
    }

    // errors return exit code 2 and output message to stderr
    for args in [vec!["1"], vec!["1", "1/2"]] {
        cmd("pk version intersect")
            .args(&args)
            .assert()
            .stderr(predicate::str::is_empty().not())
            .failure()
            .code(2);
    }
}
