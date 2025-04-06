use pkgcraft::test::{cmd, test_data};

#[test]
fn stdin() {
    // iterates over all values separated by whitespace
    cmd("pk version sort -")
        .write_stdin("2.10 2.9\n2.11 \t2.8")
        .assert()
        .stdout("2.8\n2.9\n2.10\n2.11\n");

    // invalid args
    cmd("pk version sort -")
        .write_stdin("a/b")
        .assert()
        .failure();

    // remaining args aren't ignored when "-" specified
    cmd("pk version sort -")
        .args(["2.11", "2.8"])
        .write_stdin("2.10\n2.9")
        .assert()
        .stdout("2.8\n2.9\n2.10\n2.11\n");
}

#[test]
fn args() {
    // valid args
    cmd("pk version sort")
        .args(["2.10", "2.9"])
        .assert()
        .stdout("2.9\n2.10\n");

    // invalid args
    cmd("pk version sort").arg("a/b").assert().failure();

    let data = test_data();
    for d in &data.version_toml.sorting {
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
