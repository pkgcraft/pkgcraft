use pkgcraft::test::{cmd, VersionToml};

#[test]
fn test_stdin() {
    // using stdin when no args specified
    cmd("pk version sort")
        .write_stdin("2.10 2.9")
        .assert()
        .stdout("2.9\n2.10\n");

    // iterates over all values separated by whitespace
    cmd("pk version sort")
        .write_stdin("2.10 2.9\n2.11 \t2.8")
        .assert()
        .stdout("2.8\n2.9\n2.10\n2.11\n");

    // using stdin when "-" arg specified
    cmd("pk version sort")
        .arg("-")
        .write_stdin("2.10 2.9")
        .assert()
        .stdout("2.9\n2.10\n");

    // invalid args
    cmd("pk version sort")
        .arg("-")
        .write_stdin("a/b")
        .assert()
        .failure();

    // ignoring stdin when regular args specified
    cmd("pk version sort")
        .args(["2.11", "2.8"])
        .write_stdin("2.10 2.9")
        .assert()
        .stdout("2.8\n2.11\n");
}

#[test]
fn test_args() {
    // valid args
    cmd("pk version sort")
        .args(["2.10", "2.9"])
        .assert()
        .stdout("2.9\n2.10\n");

    // invalid args
    cmd("pk version sort").arg("a/b").assert().failure();

    // use shared test data
    let data = VersionToml::load().unwrap();
    for d in data.sorting {
        let mut reversed: Vec<_> = d.sorted.clone();
        reversed.reverse();
        let output = cmd("pk version sort").args(&reversed).output().unwrap();
        let mut sorted: Vec<_> = std::str::from_utf8(&output.stdout)
            .unwrap()
            .split_whitespace()
            .collect();
        if d.equal {
            // equal objects aren't sorted so reversing should restore the original order
            sorted = sorted.into_iter().rev().collect();
        }
        assert_eq!(&sorted, &d.sorted);
    }
}
