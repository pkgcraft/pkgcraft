use std::{env, fs};

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::test_data;
use predicates::prelude::*;

use crate::cmd;

super::cmd_arg_tests!("pk pkg showkw");

#[test]
fn ignore() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();

    // invalid pkgs log errors and cause failure by default
    cmd("pk pkg showkw")
        .arg(repo)
        .assert()
        .stdout(predicate::str::is_empty().not())
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);

    // ignoring invalid pkgs entirely skips them
    for opt in ["-i", "--ignore"] {
        cmd("pk pkg showkw")
            .arg(opt)
            .arg(repo)
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}

#[test]
fn nonexistent_arches() {
    let repo = EbuildRepoBuilder::new().build().unwrap();
    env::set_current_dir(repo.path()).unwrap();
    cmd("pk pkg showkw -a arch1,arch2")
        .assert()
        .stdout("")
        .stderr("pk: error: nonexistent arches: arch1, arch2\n")
        .failure()
        .code(2);
}

#[test]
fn output() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    fs::write(repo.path().join("profiles/arch.list"), "amd64\narm64\nx86\n").unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with keywords"
        SLOT=0
        KEYWORDS="amd64 ~arm64 -x86"
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", data).unwrap();

    env::set_current_dir(repo.path()).unwrap();

    // dep restriction
    cmd("pk pkg showkw cat/pkg-1")
        .assert()
        .stdout(indoc::indoc! {"
            keywords for cat/pkg:
              │ a a   │     │
              │ m r   │ e s │ r
              │ d m x │ a l │ e
              │ 6 6 8 │ p o │ p
              │ 4 4 6 │ i t │ o
            ──┼───────┼─────┼──────
            1 │ + ~ - │ 8 0 │ test
        "})
        .stderr("")
        .success();

    // ascii format
    cmd("pk pkg showkw cat/pkg-1 -f ascii")
        .assert()
        .stdout(indoc::indoc! {"
            keywords for cat/pkg:
              | a a   |     |
              | m r   | e s | r
              | d m x | a l | e
              | 6 6 8 | p o | p
              | 4 4 6 | i t | o
            --+-------+-----+------
            1 | + ~ - | 8 0 | test
        "})
        .stderr("")
        .success();
}
