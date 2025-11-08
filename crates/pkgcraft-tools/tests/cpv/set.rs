use pkgcraft::test::test_data;

use crate::cmd;

#[test]
fn stdin() {
    // iterates over all values from stdin separated by whitespace
    cmd("pk cpv set -")
        .write_stdin("a/b-1 a/b-2\ncat/pkg-0 \ta/b-1")
        .assert()
        .stdout("a/b-1\na/b-2\ncat/pkg-0\n");

    // invalid args
    cmd("pk cpv set -").write_stdin("=a/b-1").assert().failure();

    // remaining args aren't ignored when "-" specified
    cmd("pk cpv set -")
        .args(["a/b-1", "a/b-2"])
        .write_stdin("x/y-1\ncat/pkg-2")
        .assert()
        .stdout("x/y-1\ncat/pkg-2\na/b-1\na/b-2\n");
}

#[test]
fn args() {
    // valid args
    cmd("pk cpv set")
        .args(["a/b-1", "a/b-1"])
        .assert()
        .stdout("a/b-1\n");

    // invalid args
    cmd("pk cpv set").arg("a/b").assert().failure();

    let data = test_data();
    for d in &data.version_toml.hashing {
        let cpvs: Vec<_> = d.versions.iter().map(|s| format!("cat/pkg-{s}")).collect();
        let output = cmd("pk cpv set").args(&cpvs).output().unwrap();
        let set: Vec<_> = std::str::from_utf8(&output.stdout)
            .unwrap()
            .split_whitespace()
            .collect();
        if d.equal {
            assert_eq!(set.len(), 1, "failed hashing cpvs: {cpvs:?}");
        } else {
            assert_eq!(set.len(), cpvs.len(), "failed hashing cpvs: {cpvs:?}");
        }
    }
}
