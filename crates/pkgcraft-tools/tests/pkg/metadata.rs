use std::env;

use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::Cache;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;
use tempfile::tempdir;

super::cmd_arg_tests!("pk pkg metadata");

#[test]
fn targets() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/pkg-1", &[]).unwrap();
    temp.create_ebuild("cat/pkg-2", &[]).unwrap();
    temp.create_ebuild("cat/a-1", &[]).unwrap();
    temp.create_ebuild("a/b-1", &[]).unwrap();
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();

    env::set_current_dir(&repo).unwrap();

    // Cpv target
    cmd("pk pkg metadata cat/pkg-1")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    for (cpv, status) in
        [("cat/pkg-1", true), ("cat/pkg-2", false), ("cat/a-1", false), ("a/b-1", false)]
    {
        let path = repo.metadata().cache().path().join(cpv);
        assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
    }

    // Cpn target
    cmd("pk pkg metadata cat/pkg")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    for (cpv, status) in
        [("cat/pkg-1", true), ("cat/pkg-2", true), ("cat/a-1", false), ("a/b-1", false)]
    {
        let path = repo.metadata().cache().path().join(cpv);
        assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
    }

    // category target
    cmd("pk pkg metadata cat")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    for (cpv, status) in
        [("cat/pkg-1", true), ("cat/pkg-2", true), ("cat/a-1", true), ("a/b-1", false)]
    {
        let path = repo.metadata().cache().path().join(cpv);
        assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
    }

    // repo target
    cmd("pk pkg metadata")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    for (cpv, status) in
        [("cat/pkg-1", true), ("cat/pkg-2", true), ("cat/a-1", true), ("a/b-1", true)]
    {
        let path = repo.metadata().cache().path().join(cpv);
        assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
    }
}

#[test]
fn remove() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/pkg-1", &[]).unwrap();
    temp.create_ebuild("cat/pkg-2", &[]).unwrap();
    temp.create_ebuild("cat/a-1", &[]).unwrap();
    temp.create_ebuild("a/b-1", &[]).unwrap();
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();

    env::set_current_dir(&repo).unwrap();

    for opt in ["-R", "--remove"] {
        // generate metadata
        cmd("pk pkg metadata")
            .assert()
            .stdout("")
            .stderr("")
            .success();

        // Cpv target
        cmd("pk pkg metadata cat/pkg-1")
            .arg(opt)
            .assert()
            .stdout("")
            .stderr("")
            .success();
        for (cpv, status) in
            [("cat/pkg-1", false), ("cat/pkg-2", true), ("cat/a-1", true), ("a/b-1", true)]
        {
            let path = repo.metadata().cache().path().join(cpv);
            assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
        }

        // Cpn target
        cmd("pk pkg metadata cat/pkg")
            .arg(opt)
            .assert()
            .stdout("")
            .stderr("")
            .success();
        for (cpv, status) in
            [("cat/pkg-1", false), ("cat/pkg-2", false), ("cat/a-1", true), ("a/b-1", true)]
        {
            let path = repo.metadata().cache().path().join(cpv);
            assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
        }

        // category target
        cmd("pk pkg metadata cat")
            .arg(opt)
            .assert()
            .stdout("")
            .stderr("")
            .success();
        for (cpv, status) in
            [("cat/pkg-1", false), ("cat/pkg-2", false), ("cat/a-1", false), ("a/b-1", true)]
        {
            let path = repo.metadata().cache().path().join(cpv);
            assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
        }

        // repo target
        cmd("pk pkg metadata")
            .arg(opt)
            .assert()
            .stdout("")
            .stderr("")
            .success();
        for (cpv, status) in
            [("cat/pkg-1", false), ("cat/pkg-2", false), ("cat/a-1", false), ("a/b-1", false)]
        {
            let path = repo.metadata().cache().path().join(cpv);
            assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
        }
    }
}

#[test]
fn custom_path() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/pkg-1", &[]).unwrap();
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();

    for opt in ["-p", "--path"] {
        cmd("pk pkg metadata cat/pkg-1")
            .args(["--repo", repo.as_ref()])
            .arg(opt)
            .arg(path)
            .assert()
            .stdout("")
            .stderr("")
            .success();
        // metadata files are created in the specified dir
        let metadata_path = repo.metadata().cache().path().join("cat/pkg-1");
        assert!(!metadata_path.exists());
        assert!(path.join("cat/pkg-1").exists());
    }
}

#[test]
fn verify() {
    let data = test_data();

    for opt in ["-V", "--verify"] {
        // invalid data
        let repo = data.ebuild_repo("bad").unwrap();
        cmd("pk pkg metadata")
            .arg(opt)
            .arg(repo)
            .assert()
            .stdout("")
            .stderr(predicate::str::is_empty().not())
            .failure()
            .code(2);

        // valid data
        let repo = data.ebuild_repo("metadata").unwrap();
        cmd("pk pkg metadata")
            .arg(opt)
            .arg(repo)
            .assert()
            .stdout("")
            .stderr("")
            .success();

        // verifying doesn't generate files
        let mut config = Config::default();
        let mut temp = EbuildRepoBuilder::new().build().unwrap();
        temp.create_ebuild("cat/pkg-1", &[]).unwrap();
        let repo = config
            .add_repo(&temp, false)
            .unwrap()
            .into_ebuild()
            .unwrap();
        cmd("pk pkg metadata cat/pkg-1")
            .args(["--repo", repo.as_ref()])
            .arg(opt)
            .assert()
            .stdout("")
            .stderr("")
            .success();
        let path = repo.metadata().cache().path().join("cat/pkg-1");
        assert!(!path.exists());
    }
}

#[test]
fn output() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with output during metadata generation"
        SLOT=0
        echo stdout
        echo stderr >&2
        eqawarn eqawarn
        ewarn ewarn
        eerror eerror
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    // output is suppressed by default
    cmd("pk pkg metadata")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    for opt in ["-o", "--output"] {
        cmd("pk pkg metadata -f")
            .arg(opt)
            .arg(&repo)
            .assert()
            .stdout("")
            .stderr(indoc::indoc! {"
                cat/pkg-1::test: stdout
                cat/pkg-1::test: stderr
                cat/pkg-1::test: * eqawarn
                cat/pkg-1::test: * ewarn
                cat/pkg-1::test: * eerror
            "})
            .success();
    }
}
