use pkgcraft::test::test_data;

use crate::cmd;

#[test]
fn stdin() {
    // iterates over all values from stdin separated by whitespace
    cmd("pk dep set -")
        .write_stdin("z/z a/a\nc/c \tb/b")
        .assert()
        .stdout("z/z\na/a\nc/c\nb/b\n");

    // invalid args
    cmd("pk dep set -").write_stdin("a/b/c").assert().failure();

    // remaining args aren't ignored when "-" specified
    cmd("pk dep set -")
        .args(["c/c", "b/b"])
        .write_stdin("z/z\na/a")
        .assert()
        .stdout("z/z\na/a\nc/c\nb/b\n");
}

#[test]
fn args() {
    // valid args
    cmd("pk dep set")
        .args(["a/a", "a/a"])
        .assert()
        .stdout("a/a\n");
    cmd("pk dep set")
        .args(["cat/pkg[u]", "cat/pkg[u,u]"])
        .assert()
        .stdout("cat/pkg[u]\n");

    // invalid args
    cmd("pk dep set").arg("a/b/c").assert().failure();

    let data = test_data();
    for d in &data.version_toml.hashing {
        let deps: Vec<_> = d.versions.iter().map(|s| format!("=cat/pkg-{s}")).collect();
        let output = cmd("pk dep set").args(&deps).output().unwrap();
        let set: Vec<_> = std::str::from_utf8(&output.stdout)
            .unwrap()
            .split_whitespace()
            .collect();
        if d.equal {
            assert_eq!(set.len(), 1, "failed hashing deps: {deps:?}");
        } else {
            assert_eq!(set.len(), deps.len(), "failed hashing deps: {deps:?}");
        }
    }
}
