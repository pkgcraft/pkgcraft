use std::path::PathBuf;
use std::str;
use std::sync::LazyLock;

use assert_cmd::Command as assert_command;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pretty_assertions::assert_eq;
use tempfile::Builder;

static TARGET_DIR: LazyLock<String> = LazyLock::new(|| {
    let tmp_dir = PathBuf::from(env!("CARGO_BIN_EXE_pkgcruft-gitd"));
    tmp_dir.parent().unwrap().to_str().unwrap().to_owned()
});

#[tokio::test]
async fn uds() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    git2::Repository::init(repo.path()).unwrap();

    // run from build dir
    let env: [(&str, &str); 1] = [("PATH", &TARGET_DIR)];
    let args = [repo.path().as_str()];

    let tmp_dir = Builder::new().prefix("pkgcruft.").tempdir().unwrap();
    let socket_path = tmp_dir.path().to_owned().join("pkgcruft.sock");
    let socket = socket_path.to_str().unwrap();

    let (mut service, socket) = pkgcruft_git::spawn(&socket, Some(env), Some(args), Some(5))
        .await
        .unwrap();

    let mut cmd = assert_command::cargo_bin("pkgcruft-git").unwrap();
    let output = cmd.arg("-c").arg(&socket).arg("version").output().unwrap();

    let ver = env!("CARGO_PKG_VERSION");
    let expected = format!("client: pkgcruft-git-{ver}, server: pkgcruft-gitd-{ver}");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);

    service.kill().await.unwrap();
}

#[tokio::test]
async fn tcp() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    git2::Repository::init(repo.path()).unwrap();

    // run from build dir
    let env: [(&str, &str); 1] = [("PATH", &TARGET_DIR)];
    let args = [repo.path().as_str()];

    for addr in ["127.0.0.1:0", "[::]:0"] {
        let (mut service, socket) = pkgcruft_git::spawn(addr, Some(env), Some(args), Some(5))
            .await
            .unwrap();
        let url = format!("http://{}", &socket);

        let ver = env!("CARGO_PKG_VERSION");
        let expected = format!("client: pkgcruft-git-{ver}, server: pkgcruft-gitd-{ver}");

        // verify both raw socket and url args work
        for serve_addr in [socket, url] {
            let mut cmd = assert_command::cargo_bin("pkgcruft-git").unwrap();
            let output = cmd
                .arg("-c")
                .arg(&serve_addr)
                .arg("version")
                .output()
                .unwrap();
            assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);
        }

        service.kill().await.unwrap();
    }
}

#[tokio::test]
async fn scan() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    git2::Repository::init(repo.path()).unwrap();

    // run from build dir
    let env: [(&str, &str); 1] = [("PATH", &TARGET_DIR)];
    let args = [repo.path().as_str()];

    let tmp_dir = Builder::new().prefix("pkgcruft.").tempdir().unwrap();
    let socket = tmp_dir.path().join("pkgcruft.sock");

    let (mut service, socket) = pkgcruft_git::spawn(&socket, Some(env), Some(args), Some(5))
        .await
        .unwrap();

    // empty repo
    let mut cmd = assert_command::cargo_bin("pkgcruft-git").unwrap();
    let output = cmd.arg("-c").arg(&socket).arg("scan").output().unwrap();
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "");

    // invalid pkg
    repo.create_ebuild("cat/pkg-1", &["EAPI=invalid"]).unwrap();
    let mut cmd = assert_command::cargo_bin("pkgcruft-git").unwrap();
    let output = cmd.arg("-c").arg(&socket).arg("scan").output().unwrap();
    let expected = indoc::indoc! {"
        cat/pkg
          MetadataError: version 1: unsupported EAPI: invalid
    "};
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), expected);

    service.kill().await.unwrap();
}
