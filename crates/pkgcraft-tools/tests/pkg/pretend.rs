use std::env;

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::test_data;

use crate::cmd;
use crate::predicates::lines_contain;

const SUCCESS_WITH_OUTPUT: &str = indoc::indoc! {r#"
    EAPI=8
    DESCRIPTION="ebuild with pkg_pretend success and output"
    SLOT=0

    pkg_pretend() {
        echo output123
    }
"#};

super::cmd_arg_tests!("pk pkg pretend");

#[test]
fn pkg_target_from_stdin() {
    let data = test_data();
    let repo = data.ebuild_repo("phases").unwrap();
    cmd("pk pkg pretend -")
        .args(["-r", repo.path().as_str()])
        .write_stdin("pkg-pretend/success-with-output")
        .assert()
        .stdout(lines_contain(["pkg-pretend/success-with-output-1", "output123"]))
        .stderr("")
        .success();
}

#[test]
fn path_targets() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    repo.create_ebuild_from_str("cat1/a-1", SUCCESS_WITH_OUTPUT)
        .unwrap();
    repo.create_ebuild_from_str("cat1/b-1", SUCCESS_WITH_OUTPUT)
        .unwrap();
    repo.create_ebuild_from_str("cat2/c-1", SUCCESS_WITH_OUTPUT)
        .unwrap();

    // repo path
    cmd("pk pkg pretend")
        .arg(&repo)
        .assert()
        .stdout(lines_contain(["cat1/a-1", "cat1/b-1", "cat2/c-1", "output123"]))
        .stderr("")
        .success();

    // category path
    cmd("pk pkg pretend")
        .arg(repo.path().join("cat1"))
        .assert()
        .stdout(lines_contain(["cat1/a-1", "cat1/b-1", "output123"]))
        .stderr("")
        .success();

    // package path
    cmd("pk pkg pretend")
        .arg(repo.path().join("cat2/c"))
        .assert()
        .stdout(lines_contain(["cat2/c-1", "output123"]))
        .stderr("")
        .success();

    // default current working dir
    env::set_current_dir(repo.path().join("cat2/c")).unwrap();
    cmd("pk pkg pretend")
        .assert()
        .stdout(lines_contain(["cat2/c-1", "output123"]))
        .stderr("")
        .success();
}

#[test]
fn output() {
    let data = test_data();
    let repo = data.ebuild_repo("phases").unwrap();

    // package lacking pkg_pretend() phase
    cmd("pk pkg pretend")
        .arg(repo.path().join("pkg-pretend/none"))
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // pkg_pretend() success with no output
    cmd("pk pkg pretend")
        .arg(repo.path().join("pkg-pretend/success"))
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // pkg_pretend() success with output
    cmd("pk pkg pretend")
        .arg(repo.path().join("pkg-pretend/success-with-output"))
        .assert()
        .stdout(lines_contain(["pkg-pretend/success-with-output-1", "output123"]))
        .stderr("")
        .success();

    // pkg_pretend() failure with no output
    cmd("pk pkg pretend")
        .arg(repo.path().join("pkg-pretend/failure"))
        .assert()
        .stdout("")
        .stderr(lines_contain(["pkg-pretend/failure-1"]))
        .failure()
        .code(1);

    // pkg_pretend() failure with with output
    cmd("pk pkg pretend")
        .arg(repo.path().join("pkg-pretend/failure-with-output"))
        .assert()
        .stdout("")
        .stderr(lines_contain(["pkg-pretend/failure-with-output-1", "output123"]))
        .failure()
        .code(1);
}
