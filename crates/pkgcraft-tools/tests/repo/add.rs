use pkgcraft::test::{cmd, test_data};
use predicates::str::contains;
use tempfile::tempdir;

#[test]
fn unsupported_syncers() {
    cmd("pk repo add nonexistent")
        .assert()
        .stdout("")
        .stderr(contains("no syncers available: nonexistent"))
        .failure()
        .code(2);
}

#[test]
fn nonexistent_local_repo() {
    cmd("pk repo add /path/to/repo")
        .assert()
        .stdout("")
        .stderr(contains("invalid local repo: /path/to/repo: No such file or directory"))
        .failure()
        .code(2);
}

#[test]
fn local_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("qa-primary").unwrap();
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().to_str().unwrap();

    cmd("pk repo add")
        .args(["--config", config_dir])
        .arg(repo)
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
    cmd("pk repo add")
        .args(["--config", config_dir])
        .arg("https://github.com/radhermit/radhermit-overlay.git")
        .assert()
        .stdout("")
        .stderr("")
        .success();
}
