use pkgcraft::test::{cmd, DepToml};

#[test]
fn test_stdin() {
    // using stdin when no args specified
    cmd("pk dep sort")
        .write_stdin("z/z a/a")
        .assert()
        .stdout("a/a\nz/z\n");

    // iterates over all values separated by whitespace
    cmd("pk dep sort")
        .write_stdin("z/z a/a\nc/c \tb/b")
        .assert()
        .stdout("a/a\nb/b\nc/c\nz/z\n");

    // using stdin when "-" arg specified
    cmd("pk dep sort")
        .arg("-")
        .write_stdin("z/z a/a")
        .assert()
        .stdout("a/a\nz/z\n");

    // invalid args
    cmd("pk dep sort")
        .arg("-")
        .write_stdin("a/b/c")
        .assert()
        .failure();

    // ignoring stdin when regular args specified
    cmd("pk dep sort")
        .args(["c/c", "b/b"])
        .write_stdin("z/z a/a")
        .assert()
        .stdout("b/b\nc/c\n");
}

#[test]
fn test_args() {
    // valid args
    cmd("pk dep sort")
        .args(["z/z", "a/a"])
        .assert()
        .stdout("a/a\nz/z\n");

    // invalid args
    cmd("pk dep sort").arg("a/b/c").assert().failure();

    // use shared test data
    let data = DepToml::load().unwrap();
    for d in data.sorting {
        let mut reversed: Vec<_> = d.sorted.clone();
        reversed.reverse();
        let output = cmd("pk dep sort").args(&reversed).output().unwrap();
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
