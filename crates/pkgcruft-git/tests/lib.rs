use camino::Utf8Path;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::cmd;
use predicates::prelude::*;

mod utils;

use utils::PkgcruftServiceBuilder;

/// Initialize a git repo at a path, adding all files to an initial commit.
fn init_git_repo(path: &Utf8Path) {
    let git_repo = git2::Repository::init(path).unwrap();
    let mut index = git_repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let oid = index.write_tree().unwrap();
    let tree = git_repo.find_tree(oid).unwrap();
    let sig = git2::Signature::new("test", "test@test.test", &git2::Time::new(0, 0)).unwrap();
    git_repo
        .commit(Some("HEAD"), &sig, &sig, "initial import", &tree, &[])
        .unwrap();
}

#[tokio::test]
async fn uds() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    init_git_repo(repo.path());

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
    init_git_repo(repo.path());

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
