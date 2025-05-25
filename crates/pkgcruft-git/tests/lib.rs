use std::str;

use assert_cmd::Command as assert_command;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pretty_assertions::assert_eq;

mod utils;

use utils::PkgcruftServiceBuilder;

#[tokio::test]
async fn uds() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    git2::Repository::init(repo.path()).unwrap();

    let service = PkgcruftServiceBuilder::new(repo.path()).spawn().await;
    let mut cmd = assert_command::cargo_bin("pkgcruft-git").unwrap();
    let output = cmd
        .arg("-c")
        .arg(&service.socket)
        .arg("version")
        .output()
        .unwrap();

    let ver = env!("CARGO_PKG_VERSION");
    let expected = format!("client: {ver}, server: {ver}");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);
}

#[tokio::test]
async fn tcp() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    git2::Repository::init(repo.path()).unwrap();

    for addr in ["127.0.0.1:0", "[::]:0"] {
        let service = PkgcruftServiceBuilder::new(repo.path())
            .socket(addr)
            .spawn()
            .await;
        let url = format!("http://{}", &service.socket);

        let ver = env!("CARGO_PKG_VERSION");
        let expected = format!("client: {ver}, server: {ver}");

        // verify both raw socket and url args work
        for serve_addr in [&service.socket, &url] {
            let mut cmd = assert_command::cargo_bin("pkgcruft-git").unwrap();
            let output = cmd
                .arg("-c")
                .arg(serve_addr)
                .arg("version")
                .output()
                .unwrap();
            assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);
        }
    }
}

#[tokio::test]
async fn scan() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    git2::Repository::init(repo.path()).unwrap();

    let service = PkgcruftServiceBuilder::new(repo.path()).spawn().await;

    // empty repo
    let mut cmd = assert_command::cargo_bin("pkgcruft-git").unwrap();
    let output = cmd
        .arg("-c")
        .arg(&service.socket)
        .arg("scan")
        .output()
        .unwrap();
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), "");

    // invalid pkg
    repo.create_ebuild("cat/pkg-1", &["EAPI=invalid"]).unwrap();
    let mut cmd = assert_command::cargo_bin("pkgcruft-git").unwrap();
    let output = cmd
        .arg("-c")
        .arg(&service.socket)
        .arg("scan")
        .output()
        .unwrap();
    let expected = indoc::indoc! {"
        cat/pkg
          MetadataError: version 1: unsupported EAPI: invalid
    "};
    assert_eq!(str::from_utf8(&output.stdout).unwrap(), expected);
}
