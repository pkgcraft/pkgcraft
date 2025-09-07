use pkgcraft::test::{cmd, test_data};
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn nonexistent_repo() {
    cmd("pk repo remove repo1 repo2")
        .assert()
        .stdout("")
        .stderr(contains("failed removing nonexistent repos: repo1, repo2"))
        .failure()
        .code(2);
}

#[test]
fn portage_repos() {
    cmd("pk repo remove --portage repo")
        .assert()
        .stdout("")
        .stderr(contains("config error: can't alter portage repos"))
        .failure()
        .code(2);
}

#[test]
fn remove() {
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
    cmd("pk repo remove qa-primary")
        .args(["--config", config_dir])
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // multiple
    let repo1 = data.ebuild_repo("qa-primary").unwrap();
    let repo2 = data.ebuild_repo("empty").unwrap();
    cmd("pk repo add")
        .args(["--config", config_dir])
        .args([repo1, repo2])
        .assert()
        .stdout("")
        .stderr("")
        .success();
    cmd("pk repo remove qa-primary empty")
        .args(["--config", config_dir])
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
