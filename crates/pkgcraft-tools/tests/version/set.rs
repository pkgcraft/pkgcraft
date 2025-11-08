use pkgcraft::test::test_data;

use crate::cmd;

#[test]
fn stdin() {
    // iterates over all values separated by whitespace
    cmd("pk version set -")
        .write_stdin("2.10 2.9\n2.11 \t2.8")
        .assert()
        .stdout("2.10\n2.9\n2.11\n2.8\n");

    // invalid args
    cmd("pk version set -")
        .write_stdin("a/b")
        .assert()
        .failure();

    // remaining args aren't ignored when "-" specified
    cmd("pk version set -")
        .args(["2.11", "2.8"])
        .write_stdin("2.10\n2.9")
        .assert()
        .stdout("2.10\n2.9\n2.11\n2.8\n");
}

#[test]
fn args() {
    // valid args
    cmd("pk version set")
        .args(["2.10", "2.10"])
        .assert()
        .stdout("2.10\n");

    // invalid args
    cmd("pk version sort").arg("a/b").assert().failure();

    let data = test_data();
    for d in &data.version_toml.hashing {
        let versions = &d.versions;
        let output = cmd("pk version set").args(versions).output().unwrap();
        let set: Vec<_> = std::str::from_utf8(&output.stdout)
            .unwrap()
            .split_whitespace()
            .collect();
        if d.equal {
            assert_eq!(set.len(), 1, "failed hashing versions: {versions:?}");
        } else {
            assert_eq!(set.len(), versions.len(), "failed hashing versions: {versions:?}");
        }
    }
}
