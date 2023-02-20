use pkgcraft::test::{cmd, VersionToml};
use predicates::prelude::*;

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
            .failure()
            .code(2)
            .stderr(predicate::str::is_empty().not());
    }
}

#[test]
fn toml() {
    let data = VersionToml::load().unwrap();
    for d in data.intersects {
        let (s1, s2) = (d.vals[0].as_str(), d.vals[1].as_str());

        // elements intersect themselves
        cmd("pk version intersect").args([s1, s1]).assert();
        cmd("pk version intersect").args([s2, s2]).assert();

        // intersects depending on status
        let status = cmd("pk version intersect").args([s1, s2]).assert();
        if d.status {
            status.success();
        } else {
            status.failure().code(1);
        }
    }
}
