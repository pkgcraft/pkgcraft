use pkgcraft::test::{cmd, VersionToml};

#[test]
fn stdin() {
    // using stdin when no args specified
    cmd("pk version set")
        .write_stdin("2.10 2.9")
        .assert()
        .stdout("2.10\n2.9\n");

    // iterates over all values separated by whitespace
    cmd("pk version set")
        .write_stdin("2.10 2.9\n2.11 \t2.8")
        .assert()
        .stdout("2.10\n2.9\n2.11\n2.8\n");

    // using stdin when "-" arg specified
    cmd("pk version set")
        .arg("-")
        .write_stdin("2.10 2.9")
        .assert()
        .stdout("2.10\n2.9\n");

    // invalid args
    cmd("pk version set")
        .arg("-")
        .write_stdin("a/b")
        .assert()
        .failure();

    // ignoring stdin when regular args specified
    cmd("pk version set")
        .args(["2.11", "2.8"])
        .write_stdin("2.10 2.9")
        .assert()
        .stdout("2.11\n2.8\n");
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

    // use shared test data
    let data = VersionToml::load().unwrap();
    for d in data.hashing {
        let versions = d.versions;
        let output = cmd("pk version set").args(&versions).output().unwrap();
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
