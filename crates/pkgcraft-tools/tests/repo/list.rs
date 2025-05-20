use pkgcraft::test::{cmd, test_data};
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn no_repos() {
    cmd("pk repo list").assert().stdout("").stderr("").success();
}

#[test]
fn names() {
    let data = test_data();
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().to_str().unwrap();

    // single
    let repo = data.ebuild_repo("qa-primary").unwrap();
    cmd("pk repo add")
        .args(["--config", config_dir])
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    cmd("pk repo list")
        .args(["--config", config_dir])
        .assert()
        .stdout(indoc::indoc! {"
            qa-primary
        "})
        .stderr("")
        .success();

    // multiple
    let repo1 = data.ebuild_repo("qa-secondary").unwrap();
    let repo2 = data.ebuild_repo("empty").unwrap();
    cmd("pk repo add")
        .args(["--config", config_dir])
        .args([repo1, repo2])
        .assert()
        .stdout("")
        .stderr("")
        .success();
    cmd("pk repo list")
        .args(["--config", config_dir])
        .assert()
        .stdout(indoc::indoc! {"
            empty
            qa-primary
            qa-secondary
        "})
        .stderr("")
        .success();
}

#[test]
fn path() {
    let data = test_data();
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().to_str().unwrap();

    let repo = data.ebuild_repo("qa-primary").unwrap();
    cmd("pk repo add")
        .args(["--config", config_dir])
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    for opt in ["-p", "--path"] {
        cmd("pk repo list")
            .args(["--config", config_dir])
            .arg(opt)
            .assert()
            .stdout(contains("qa-primary"))
            .stderr("")
            .success();
    }
}

#[test]
fn full() {
    let data = test_data();
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().to_str().unwrap();

    let repo = data.ebuild_repo("qa-primary").unwrap();
    cmd("pk repo add")
        .args(["--config", config_dir])
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    for opt in ["-f", "--full"] {
        cmd("pk repo list")
            .args(["--config", config_dir])
            .arg(opt)
            .assert()
            .stdout(contains("qa-primary"))
            .stderr("")
            .success();
    }
}
