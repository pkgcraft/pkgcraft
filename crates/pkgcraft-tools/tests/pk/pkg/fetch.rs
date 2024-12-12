use std::{env, fs};

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn invalid_cwd_target() {
    cmd("pk pkg fetch")
        .assert()
        .stdout("")
        .stderr(contains("invalid ebuild repo"))
        .failure();
}

#[test]
fn nonexistent_path_target() {
    let repo = "path/to/nonexistent/repo";
    cmd(format!("pk pkg fetch {repo}"))
        .assert()
        .stdout("")
        .stderr(contains(format!("invalid path target: {repo}: No such file or directory")))
        .failure();
}

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    cmd("pk pkg fetch")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
}

#[test]
fn invalid_pkgs() {
    let data = test_data();
    let repo = data.ebuild_repo("bad").unwrap();
    cmd("pk pkg fetch")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);
}

#[tokio::test]
async fn nonexistent() {
    let server = MockServer::start().await;
    let uri = server.uri();

    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with nonexistent URI"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
    let repo = temp.path();

    cmd("pk pkg fetch")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(contains(format!("failed to get: {uri}/file")))
        .failure()
        .code(1);
}

#[tokio::test]
async fn fetch() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test1"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/file2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test2"))
        .mount(&server)
        .await;

    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file1"
        SLOT=0
    "#};
    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file2"
        SLOT=0
    "#};
    temp.create_ebuild_from_str("cat/pkg-2", &data).unwrap();
    let repo = temp.path();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    // version scope
    cmd("pk pkg fetch")
        .arg(repo.join("cat/pkg/pkg-1.ebuild"))
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file1").unwrap();
    assert_eq!(&data, "test1");

    // package scope
    cmd("pk pkg fetch")
        .arg(repo.join("cat/pkg"))
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file2").unwrap();
    assert_eq!(&data, "test2");
}

#[tokio::test]
async fn custom_mirror() {
    let server = MockServer::start().await;
    let uri = server.uri();
    let name = "mocked";

    Mock::given(method("GET"))
        .and(path("/file1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test1"))
        .mount(&server)
        .await;

    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with custom mirror"
        SRC_URI="mirror://{name}/file1"
        SLOT=0
    "#};
    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
    let repo = temp.path();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    // unfetchable URIs are currently ignored
    cmd("pk pkg fetch")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    assert!(fs::read_to_string("file1").is_err());

    // register mocked mirror
    fs::create_dir_all(repo.join("profiles")).unwrap();
    fs::write(repo.join("profiles/thirdpartymirrors"), format!("{name} {uri}")).unwrap();

    // mirror resolves to fetchable URI
    cmd("pk pkg fetch")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file1").unwrap();
    assert_eq!(&data, "test1");
}
