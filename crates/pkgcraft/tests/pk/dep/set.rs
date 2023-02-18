use pkgcraft::test::{cmd, VersionToml};

#[test]
fn stdin() {
    // using stdin when no args specified
    cmd("pk dep set")
        .write_stdin("z/z a/a")
        .assert()
        .stdout("z/z\na/a\n");

    // iterates over all values separated by whitespace
    cmd("pk dep set")
        .write_stdin("z/z a/a\nc/c \tb/b")
        .assert()
        .stdout("z/z\na/a\nc/c\nb/b\n");

    // using stdin when "-" arg specified
    cmd("pk dep set")
        .arg("-")
        .write_stdin("z/z a/a")
        .assert()
        .stdout("z/z\na/a\n");

    // invalid args
    cmd("pk dep set")
        .arg("-")
        .write_stdin("a/b/c")
        .assert()
        .failure();

    // ignoring stdin when regular args specified
    cmd("pk dep set")
        .args(["c/c", "b/b"])
        .write_stdin("z/z a/a")
        .assert()
        .stdout("c/c\nb/b\n");
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

    // use shared test data
    let data = VersionToml::load().unwrap();
    for d in data.hashing {
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
