use std::env;

use pkgcraft::repo::ebuild::temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::prelude::*;
use predicates::str::contains;

use crate::predicates::lines_contain;

#[test]
fn invalid_cwd_target() {
    cmd("pk pkg source")
        .assert()
        .stdout("")
        .stderr(contains("invalid repo path"))
        .failure()
        .code(2);
}

#[test]
fn nonexistent_path_target() {
    let path = "path/to/nonexistent/repo";
    cmd(format!("pk pkg source {path}"))
        .assert()
        .stdout("")
        .stderr(contains(format!("invalid path target: {path}: No such file or directory")))
        .failure()
        .code(2);
}

#[test]
fn no_pkgs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd("pk pkg source")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn pkg_target_from_stdin() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat/dep-1", &[]).unwrap();
    cmd("pk pkg source -")
        .write_stdin(format!("cat/dep::{}", t.path()))
        .assert()
        .stdout(lines_contain(["cat/dep-1"]))
        .stderr("")
        .success();
}

#[test]
fn invalid_pkgs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    let path = t.path();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with global die"
        SLOT=0
        die msg
    "#};
    t.create_raw_pkg_from_str("a/pkg-1", &data).unwrap();
    t.create_raw_pkg_from_str("cat/a-1", &data).unwrap();
    t.create_raw_pkg_from_str("cat/b-1", &data).unwrap();

    // dep restriction
    cmd(format!("pk pkg source cat/a-1::{path}"))
        .assert()
        .stdout("")
        .stderr(lines_contain([format!("invalid pkg: cat/a-1::{path}: line 4: die: error: msg")]))
        .failure()
        .code(1);

    // category restriction
    cmd(format!("pk pkg source cat/*::{path}"))
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat/a-1", "cat/b-1"]))
        .failure()
        .code(1);

    // repo target
    cmd(format!("pk pkg source {}", path))
        .assert()
        .stdout("")
        .stderr(lines_contain(["a/pkg-1", "cat/a-1", "cat/b-1"]))
        .failure()
        .code(1);

    // benchmarking failures
    cmd(format!("pk pkg source --bench 500ms {}", path))
        .assert()
        .stdout("")
        .stderr(lines_contain(["a/pkg-1", "cat/a-1", "cat/b-1"]))
        .failure()
        .code(1);
}

#[test]
fn path_targets() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat1/a-1", &[]).unwrap();
    t.create_raw_pkg("cat1/b-1", &[]).unwrap();
    t.create_raw_pkg("cat2/c-1", &[]).unwrap();

    // repo path
    cmd("pk pkg source")
        .arg(t.path())
        .assert()
        .stdout(lines_contain(["cat1/a-1", "cat1/b-1", "cat2/c-1"]))
        .stderr("")
        .success();

    // category path
    cmd("pk pkg source")
        .arg(t.path().join("cat1"))
        .assert()
        .stdout(lines_contain(["cat1/a-1", "cat1/b-1"]))
        .stderr("")
        .success();

    // package path
    cmd("pk pkg source")
        .arg(t.path().join("cat2/c"))
        .assert()
        .stdout(lines_contain(["cat2/c-1"]))
        .stderr("")
        .success();

    // default current working dir
    env::set_current_dir(t.path().join("cat2/c")).unwrap();
    cmd("pk pkg source")
        .assert()
        .stdout(lines_contain(["cat2/c-1"]))
        .stderr("")
        .success();
}

#[test]
#[cfg(feature = "flaky")]
fn bound_and_sort() {
    use std::os::fd::AsRawFd;

    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg("cat/fast-1", &[]).unwrap();
    let f = std::fs::File::open(t.path().join("profiles/repo_name")).unwrap();
    let fd = f.as_raw_fd();

    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with slower global scope code"
        SLOT=0

        # forcibly wait for at least 25ms to slow down ebuild sourcing
        read -t 0.025 -u {fd}

        :
    "#};
    t.create_raw_pkg_from_str("cat/slow-1", &data).unwrap();

    for opt in ["-b", "--bound"] {
        for (val, pkg) in [
            ("25ms", "cat/slow"),
            (">25ms", "cat/slow"),
            (">=25ms", "cat/slow"),
            ("<25ms", "cat/fast"),
            ("<=25ms", "cat/fast"),
        ] {
            cmd("pk pkg source")
                .args([opt, val])
                .arg(t.path())
                .assert()
                .stdout(lines_contain([pkg]))
                .stderr("")
                .success();
        }
    }

    // sorting output
    for opts in [vec![], vec!["--bench", "500ms"]] {
        cmd("pk pkg source --sort")
            .args(opts)
            .arg(t.path())
            .assert()
            .stdout(predicate::function(|s: &str| {
                let lines: Vec<_> = s
                    .lines()
                    .filter_map(|s| s.split_once("::"))
                    .map(|(x, _)| x)
                    .collect();
                assert_eq!(lines, ["cat/fast-1", "cat/slow-1"]);
                true
            }))
            .stderr("")
            .success();
    }
}
