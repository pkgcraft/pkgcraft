use pkgcraft::test::test_data;

use crate::cmd;

#[test]
fn stdin() {
    // iterates over all values separated by whitespace
    cmd("pk dep sort -")
        .write_stdin("z/z a/a\nc/c \tb/b")
        .assert()
        .stdout("a/a\nb/b\nc/c\nz/z\n");

    // invalid args
    cmd("pk dep sort -").write_stdin("a/b/c").assert().failure();

    // remaining args aren't ignored when "-" specified
    cmd("pk dep sort -")
        .args(["c/c", "b/b"])
        .write_stdin("z/z\na/a")
        .assert()
        .stdout("a/a\nb/b\nc/c\nz/z\n");
}

#[test]
fn args() {
    // valid args
    cmd("pk dep sort")
        .args(["z/z", "a/a"])
        .assert()
        .stdout("a/a\nz/z\n");

    // invalid args
    cmd("pk dep sort").arg("a/b/c").assert().failure();

    let data = test_data();
    for d in &data.dep_toml.sorting {
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
