use std::os::fd::AsRawFd;
use std::{env, fs};

use pkgcraft::repo::ebuild_temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::prelude::*;
use predicates::function::FnPredicate;
use predicates::str::contains;

type FnPredStr = dyn Fn(&str) -> bool;

fn output_lines(n: usize) -> FnPredicate<Box<FnPredStr>, str> {
    predicate::function(Box::new(move |s: &str| s.lines().count() == n))
}

#[test]
fn invalid_cwd_target() {
    cmd("pk pkg source")
        .assert()
        .stdout("")
        .stderr(contains("non-ebuild repo"))
        .failure()
        .code(2);
}

#[test]
fn nonexistent_path_target() {
    cmd("pk pkg source path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("invalid dep restriction"))
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
    t.create_ebuild("cat/dep-1", &[]).unwrap();
    cmd(format!("pk pkg source -r {} -", t.path()))
        .write_stdin("cat/dep")
        .assert()
        .stdout(output_lines(1))
        .stderr("")
        .success();
}

#[test]
fn path_targets() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat1/a-1", &[]).unwrap();
    t.create_ebuild("cat1/b-1", &[]).unwrap();
    t.create_ebuild("cat2/c-1", &[]).unwrap();

    // repo path
    cmd("pk pkg source")
        .arg(t.path())
        .assert()
        .stdout(output_lines(3))
        .stderr("")
        .success();

    // category path
    cmd("pk pkg source")
        .arg(t.path().join("cat1"))
        .assert()
        .stdout(output_lines(2))
        .stderr("")
        .success();

    // package path
    cmd("pk pkg source")
        .arg(t.path().join("cat2/c"))
        .assert()
        .stdout(output_lines(1).and(contains("cat2/c-1")))
        .stderr("")
        .success();

    // default current working dir
    env::set_current_dir(t.path().join("cat2/c")).unwrap();
    cmd("pk pkg source")
        .assert()
        .stdout(output_lines(1).and(contains("cat2/c-1")))
        .stderr("")
        .success();
}

#[test]
fn bound() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_ebuild("cat/fast-1", &[]).unwrap();
    let f = fs::File::open(t.path().join("profiles/repo_name")).unwrap();
    let fd = f.as_raw_fd();

    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with slower global scope code"
        SLOT=0

        # forcibly wait for at least 25ms to slow down ebuild sourcing
        read -t 0.025 -u {fd}

        :
    "#};
    t.create_ebuild_raw("cat/slow-1", &data).unwrap();

    for opt in ["-b", "--bound"] {
        for (val, pkg) in [("25ms", "cat/slow"), (">25ms", "cat/slow"), ("<25ms", "cat/fast")] {
            cmd("pk pkg source")
                .args([opt, val])
                .arg(t.path())
                .assert()
                .stdout(output_lines(1).and(contains(pkg)))
                .stderr("")
                .success();
        }
    }
}
