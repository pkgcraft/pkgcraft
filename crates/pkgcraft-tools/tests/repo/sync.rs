use pkgcraft::test::test_data;
use predicates::str::contains;
use tempfile::tempdir;

use crate::cmd;

#[test]
fn nonexistent_repo() {
    cmd("pk repo sync nonexistent")
        .assert()
        .stdout("")
        .stderr(contains("nonexistent repo: nonexistent"))
        .failure()
        .code(2);
}

#[test]
fn portage_repos() {
    cmd("pk repo sync --portage repo")
        .assert()
        .stdout("")
        .stderr(contains("config error: can't alter portage repos"))
        .failure()
        .code(2);
}

#[test]
fn no_repos() {
    cmd("pk repo sync").assert().stdout("").stderr("").success();
}

#[test]
fn local_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().to_str().unwrap();

    cmd("pk repo add -f -n test")
        .args(["--config", config_dir])
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // initial sync
    cmd("pk repo sync test")
        .args(["--config", config_dir])
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // sync of existing repo
    cmd("pk repo sync test")
        .args(["--config", config_dir])
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
#[cfg(feature = "network")]
fn git_repo() {
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().to_str().unwrap();

    // TODO: replace with pkgcraft stub ebuild repo
    cmd("pk repo add -f -n test")
        .args(["--config", config_dir])
        .arg("https://github.com/radhermit/radhermit-overlay.git")
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // initial sync will work
    cmd("pk repo sync test")
        .args(["--config", config_dir])
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // syncing existing repo will error out on config load
    cmd("pk repo sync test")
        .args(["--config", config_dir])
        .assert()
        .stdout("")
        .stderr("pk: error: config error: test: nonexistent masters: gentoo\n")
        .failure()
        .code(2);
}
