use std::env;

use pkgcraft::repo::ebuild::temp::Repo as TempRepo;
use pkgcraft::test::cmd;
use predicates::str::contains;

use crate::predicates::lines_contain;

const NO_PKG_PRETEND: &str = indoc::indoc! {r#"
    EAPI=8
    DESCRIPTION="ebuild without pkg_pretend"
    SLOT=0
"#};

const SUCCESS: &str = indoc::indoc! {r#"
    EAPI=8
    DESCRIPTION="ebuild with pkg_pretend success"
    SLOT=0
    pkg_pretend() { :; }
"#};

const SUCCESS_WITH_OUTPUT: &str = indoc::indoc! {r#"
    EAPI=8
    DESCRIPTION="ebuild with pkg_pretend success and output"
    SLOT=0

    pkg_pretend() {
        echo output123
    }
"#};

const FAILURE: &str = indoc::indoc! {r#"
    EAPI=8
    DESCRIPTION="ebuild with pkg_pretend failure"
    SLOT=0

    pkg_pretend() {
        return 1
    }
"#};

const FAILURE_WITH_OUTPUT: &str = indoc::indoc! {r#"
    EAPI=8
    DESCRIPTION="ebuild with pkg_pretend failure and output"
    SLOT=0

    pkg_pretend() {
        echo output123
        return 1
    }
"#};

#[test]
fn invalid_cwd_target() {
    cmd("pk pkg pretend")
        .assert()
        .stdout("")
        .stderr(contains("non-ebuild repo"))
        .failure()
        .code(2);
}

#[test]
fn nonexistent_path_target() {
    cmd("pk pkg pretend path/to/nonexistent/repo")
        .assert()
        .stdout("")
        .stderr(contains("invalid dep restriction"))
        .failure()
        .code(2);
}

#[test]
fn no_pkgs() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    cmd("pk pkg pretend")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn pkg_target_from_stdin() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg_from_str("cat/dep-1", SUCCESS_WITH_OUTPUT)
        .unwrap();
    cmd(format!("pk pkg pretend -r {} -", t.path()))
        .write_stdin("cat/dep")
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat/dep-1", "output123"]))
        .success();
}

#[test]
fn path_targets() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg_from_str("cat1/a-1", SUCCESS_WITH_OUTPUT)
        .unwrap();
    t.create_raw_pkg_from_str("cat1/b-1", SUCCESS_WITH_OUTPUT)
        .unwrap();
    t.create_raw_pkg_from_str("cat2/c-1", SUCCESS_WITH_OUTPUT)
        .unwrap();

    // repo path
    cmd("pk pkg pretend")
        .arg(t.path())
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat1/a-1", "cat1/b-1", "cat2/c-1", "output123"]))
        .success();

    // category path
    cmd("pk pkg pretend")
        .arg(t.path().join("cat1"))
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat1/a-1", "cat1/b-1", "output123"]))
        .success();

    // package path
    cmd("pk pkg pretend")
        .arg(t.path().join("cat2/c"))
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat2/c-1", "output123"]))
        .success();

    // default current working dir
    env::set_current_dir(t.path().join("cat2/c")).unwrap();
    cmd("pk pkg pretend")
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat2/c-1", "output123"]))
        .success();
}

#[test]
fn output() {
    let t = TempRepo::new("test", None, 0, None).unwrap();
    t.create_raw_pkg_from_str("cat/none-1", NO_PKG_PRETEND)
        .unwrap();
    t.create_raw_pkg_from_str("cat/success-1", SUCCESS).unwrap();
    t.create_raw_pkg_from_str("cat/success-with-output-1", SUCCESS_WITH_OUTPUT)
        .unwrap();
    t.create_raw_pkg_from_str("cat/failure-1", FAILURE).unwrap();
    t.create_raw_pkg_from_str("cat/failure-with-output-1", FAILURE_WITH_OUTPUT)
        .unwrap();

    // package lacking pkg_pretend() phase
    cmd("pk pkg pretend")
        .arg(t.path().join("cat/none"))
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // pkg_pretend() success with no output
    cmd("pk pkg pretend")
        .arg(t.path().join("cat/success"))
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // pkg_pretend() success with output
    cmd("pk pkg pretend")
        .arg(t.path().join("cat/success-with-output"))
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat/success-with-output-1", "output123"]))
        .success();

    // pkg_pretend() failure with no output
    cmd("pk pkg pretend")
        .arg(t.path().join("cat/failure"))
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat/failure-1"]))
        .failure()
        .code(1);

    // pkg_pretend() failure with with output
    cmd("pk pkg pretend")
        .arg(t.path().join("cat/failure-with-output"))
        .assert()
        .stdout("")
        .stderr(lines_contain(["cat/failure-with-output-1", "output123"]))
        .failure()
        .code(1);
}
