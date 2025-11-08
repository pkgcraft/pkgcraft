use pkgcraft::dep::Version;
use pkgcraft::test::test_data;

use crate::cmd;

#[test]
fn stdin() {
    // iterates over all values separated by whitespace
    cmd("pk cpv sort -")
        .write_stdin("z/z-1 a/a-2\nc/c-1 \ta/a-1")
        .assert()
        .stdout("a/a-1\na/a-2\nc/c-1\nz/z-1\n");

    // invalid args
    cmd("pk cpv sort -")
        .write_stdin("=a/b-1")
        .assert()
        .failure();

    // remaining args aren't ignored when "-" specified
    cmd("pk cpv sort -")
        .args(["c/c-1", "b/b-1"])
        .write_stdin("z/z-1\na/a-1")
        .assert()
        .stdout("a/a-1\nb/b-1\nc/c-1\nz/z-1\n");
}

#[test]
fn args() {
    // valid args
    cmd("pk cpv sort")
        .args(["z/z-1", "a/a-1"])
        .assert()
        .stdout("a/a-1\nz/z-1\n");

    // invalid args
    cmd("pk cpv sort").arg("a/b").assert().failure();

    let data = test_data();
    for d in &data.version_toml.sorting {
        // use all versions without operators
        let versions: Vec<_> = d
            .sorted
            .iter()
            .filter_map(|s| Version::try_new(s).ok())
            .filter(|v| v.op().is_none())
            .collect();
        if !versions.is_empty() {
            let mut reversed: Vec<_> =
                versions.iter().map(|v| format!("cat/pkg-{v}")).collect();
            reversed.reverse();
            let output = cmd("pk cpv sort").args(&reversed).output().unwrap();
            let mut sorted: Vec<_> = std::str::from_utf8(&output.stdout)
                .unwrap()
                .split_whitespace()
                .filter_map(|s| s.strip_prefix("cat/pkg-"))
                .collect();
            if d.equal {
                // equal objects aren't sorted so reversing should restore the original order
                sorted = sorted.into_iter().rev().collect();
            }
            assert_eq!(&sorted, &d.sorted);
        }
    }
}
