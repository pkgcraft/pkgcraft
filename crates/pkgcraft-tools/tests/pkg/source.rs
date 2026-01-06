use std::env;

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::test_data;
use predicates::prelude::*;

use crate::cmd;
use crate::predicates::lines_contain;

super::cmd_arg_tests!("pk pkg source");

#[test]
fn pkg_target_from_stdin() {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();
    cmd("pk pkg source -")
        .args(["-r", repo.path().as_str()])
        .write_stdin("slot/slot")
        .assert()
        .stdout(lines_contain(["slot/slot-8"]))
        .stderr("")
        .success();
}

#[test]
fn invalid_pkgs() {
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with global die"
        SLOT=0
        die msg
    "#};
    temp.create_ebuild_from_str("a/pkg-1", &data).unwrap();
    temp.create_ebuild_from_str("cat/a-1", &data).unwrap();
    temp.create_ebuild_from_str("cat/b-1", &data).unwrap();
    let path = temp.path();

    // dep restriction
    cmd("pk pkg source")
        .args(["-r", path.as_str()])
        .arg("cat/a-1")
        .assert()
        .stdout("")
        .stderr(lines_contain(["invalid pkg: cat/a-1::test: line 4: die: error: msg"]))
        .failure()
        .code(1);

    // category restriction
    cmd("pk pkg source")
        .args(["-r", path.as_str()])
        .arg("cat/*")
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat/a-1", "cat/b-1"]))
        .failure()
        .code(1);

    // repo level
    for args in [vec![], vec!["-b", "500ms"], vec!["-c", "1"]] {
        cmd(format!("pk pkg source {path}"))
            .args(args)
            .assert()
            .stderr(lines_contain(["a/pkg-1", "cat/a-1", "cat/b-1"]))
            .failure()
            .code(1);
    }
}

#[test]
fn jobs() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    repo.create_ebuild("cat/pkg-1", &[]).unwrap();

    for opt in ["-j", "--jobs"] {
        // invalid
        for val in ["", "-1"] {
            cmd("pk pkg source")
                .args([opt, val])
                .assert()
                .stdout("")
                .stderr(predicate::str::is_empty().not())
                .failure()
                .code(2);
        }

        // valid and automatically bounded between 1 and max CPUs
        for val in ["0", "999999"] {
            cmd("pk pkg source")
                .args([opt, val])
                .arg(&repo)
                .assert()
                .stdout(predicate::str::is_empty().not())
                .stderr("")
                .success();
        }
    }
}

#[test]
fn path_targets() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    repo.create_ebuild("cat1/a-1", &[]).unwrap();
    repo.create_ebuild("cat1/b-1", &[]).unwrap();
    repo.create_ebuild("cat2/c-1", &[]).unwrap();

    // repo path
    cmd("pk pkg source")
        .arg(&repo)
        .assert()
        .stdout(lines_contain(["cat1/a-1", "cat1/b-1", "cat2/c-1"]))
        .stderr("")
        .success();

    // category path
    cmd("pk pkg source")
        .arg(repo.path().join("cat1"))
        .assert()
        .stdout(lines_contain(["cat1/a-1", "cat1/b-1"]))
        .stderr("")
        .success();

    // package path
    cmd("pk pkg source")
        .arg(repo.path().join("cat2/c"))
        .assert()
        .stdout(lines_contain(["cat2/c-1"]))
        .stderr("")
        .success();

    // default current working dir
    env::set_current_dir(repo.path().join("cat2/c")).unwrap();
    cmd("pk pkg source")
        .assert()
        .stdout(lines_contain(["cat2/c-1"]))
        .stderr("")
        .success();
}

#[test]
fn bound() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    repo.create_ebuild("fast/pkg-1", &[]).unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="slow sourced ebuild"
        SLOT=0
        sleep 100ms
    "#};
    repo.create_ebuild_from_str("slow/pkg-1", &data).unwrap();

    for opt in ["-B", "--bound"] {
        for (val, pkg) in [
            ("100ms", "slow/pkg"),
            (">100ms", "slow/pkg"),
            (">=100ms", "slow/pkg"),
            ("<100ms", "fast/pkg"),
            ("<=100ms", "fast/pkg"),
        ] {
            cmd("pk pkg source")
                .args([opt, val])
                .arg(&repo)
                .assert()
                .stdout(lines_contain([pkg]))
                .stderr("")
                .success();
        }
    }
}

#[test]
fn sort() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    repo.create_ebuild("z/fast-pkg-1", &[]).unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="slower sourced ebuild"
        SLOT=0
        sleep 100ms
    "#};
    repo.create_ebuild_from_str("slower/pkg-1", &data).unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="slowest sourced ebuild"
        SLOT=0
        sleep 300ms
    "#};
    repo.create_ebuild_from_str("slowest/pkg-1", &data).unwrap();

    // verify pkgs are sorted by duration
    let sorted = |s: &str| -> bool {
        let lines: Vec<_> = s.lines().collect();
        let pkgs: Vec<_> = lines
            .iter()
            .filter_map(|s| s.split_once("::"))
            .map(|(prefix, _)| prefix)
            .collect();
        pkgs == ["z/fast-pkg-1", "slower/pkg-1", "slowest/pkg-1"]
    };

    cmd("pk pkg source --sort")
        .arg(&repo)
        .assert()
        .stdout(predicate::function(sorted))
        .stderr("")
        .success();

    cmd("pk pkg source --sort --bench 2")
        .arg(&repo)
        .assert()
        .stdout(predicate::function(sorted))
        .stderr("")
        .success();
}

#[test]
fn bench() {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();

    for opt in ["-b", "--bench"] {
        // invalid
        cmd("pk pkg source")
            .arg(repo)
            .args([opt, "a"])
            .assert()
            .stdout("")
            .stderr(predicate::str::is_empty().not())
            .failure()
            .code(2);

        // default runs
        cmd("pk pkg source")
            .arg(repo)
            .arg(opt)
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();

        // valid duration
        cmd("pk pkg source")
            .arg(repo)
            .args([opt, "50ms"])
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();

        // valid runs
        cmd("pk pkg source")
            .arg(repo)
            .args([opt, "3"])
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}

#[test]
fn cumulative() {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();

    for opt in ["-c", "--cumulative"] {
        // single run without argument
        cmd("pk pkg source")
            .arg(repo)
            .arg(opt)
            .assert()
            .stdout(lines_contain(["run #1"]))
            .stderr("")
            .success();

        // multiple runs
        cmd("pk pkg source")
            .arg(repo)
            .args([opt, "3"])
            .assert()
            .stdout(lines_contain(["run #1:", "run #2:", "run #3:", "total:"]))
            .stderr("")
            .success();
    }
}

#[test]
fn format() {
    let data = test_data();
    let repo = data.ebuild_repo("metadata").unwrap();

    for opt in ["-f", "--format"] {
        // invalid
        cmd("pk pkg source")
            .args([opt, "invalid"])
            .arg(repo)
            .assert()
            .stdout("")
            .stderr(predicate::str::is_empty().not())
            .failure()
            .code(2);

        // plain
        cmd("pk pkg source")
            .args([opt, "plain"])
            .arg(repo)
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();

        // csv
        cmd("pk pkg source")
            .args([opt, "csv"])
            .arg(repo)
            .assert()
            .stdout(predicate::str::is_empty().not())
            .stderr("")
            .success();
    }
}
