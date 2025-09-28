use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::cmd;
use predicates::prelude::*;

mod git;
mod pre_commit;
mod pre_push;
mod utils;

use git::GitRepo;
use utils::PkgcruftServiceBuilder;

#[tokio::test]
async fn uds() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    GitRepo::init(&repo).unwrap();

    let service = PkgcruftServiceBuilder::new(repo.path()).spawn().await;
    let ver = env!("CARGO_PKG_VERSION");
    let expected = format!("client: {ver}, server: {ver}");
    cmd("pkgcruft-git")
        .arg("-c")
        .arg(&service.socket)
        .arg("version")
        .assert()
        .stdout(predicate::str::diff(expected).trim())
        .stderr("")
        .success();
}

#[tokio::test]
async fn tcp() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    GitRepo::init(&repo).unwrap();

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
            cmd("pkgcruft-git")
                .arg("-c")
                .arg(serve_addr)
                .arg("version")
                .assert()
                .stdout(predicate::str::diff(expected.clone()).trim())
                .stderr("")
                .success();
        }
    }
}

// TODO: fix failures due to stream disconnects under testing
/*#[tokio::test]
async fn scan() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    git2::Repository::init(repo.path()).unwrap();

    let service = PkgcruftServiceBuilder::new(repo.path()).spawn().await;

    // empty repo
    cmd("pkgcruft-git")
        .arg("-c")
        .arg(&service.socket)
        .arg("scan")
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // invalid pkg
    repo.create_ebuild("cat/pkg-1", &["EAPI=invalid"]).unwrap();
    cmd("pkgcruft-git")
        .arg("-c")
        .arg(&service.socket)
        .arg("scan")
        .assert()
        .stdout(indoc::indoc! {"
            cat/pkg
              MetadataError: version 1: unsupported EAPI: invalid
        "})
        .stderr("")
        .success();
}*/
