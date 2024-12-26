use std::env;

use pkgcraft::config::Config;
use pkgcraft::repo::ebuild::cache::Cache;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::cmd;

super::cmd_arg_tests!("pk pkg metadata");

#[test]
fn targets() {
    let mut config = Config::default();
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    temp.create_ebuild("cat/pkg-1", &[]).unwrap();
    temp.create_ebuild("cat/pkg-2", &[]).unwrap();
    temp.create_ebuild("cat/a-1", &[]).unwrap();
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
    for (cpv, status) in [("cat/pkg-1", true), ("cat/pkg-2", false), ("cat/a-1", false)] {
        let path = repo.metadata().cache().path().join(cpv);
        assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
    }

    // Cpn target
    cmd("pk pkg metadata cat/pkg")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    for (cpv, status) in [("cat/pkg-1", true), ("cat/pkg-2", true), ("cat/a-1", false)] {
        let path = repo.metadata().cache().path().join(cpv);
        assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
    }

    // category target
    cmd("pk pkg metadata cat")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    for (cpv, status) in [("cat/pkg-1", true), ("cat/pkg-2", true), ("cat/a-1", true)] {
        let path = repo.metadata().cache().path().join(cpv);
        assert_eq!(path.exists(), status, "failed for {cpv}: {path}");
    }
}
