use std::time::Duration;
use std::{env, fs};

use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::test_data;
use predicates::prelude::*;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::cmd;

super::cmd_arg_tests!("pk pkg fetch");

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

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with nonexistent URI"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr(contains(format!("fetch failed: {uri}/file: 404 Not Found")))
        .failure()
        .code(1);
}

#[tokio::test]
async fn unsupported() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with unsupported URI"
        SRC_URI="ftp://pkgcraft.pkgcraft/file"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    // FTP is not supported
    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr(contains(
            "invalid fetchable: unsupported protocol: ftp://pkgcraft.pkgcraft/file",
        ))
        .failure()
        .code(1);
}

#[tokio::test]
async fn concurrent() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file1"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/file2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file2"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/file3"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file3"))
        .mount(&server)
        .await;

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="u1? ( {uri}/file1 ) u2? ( {uri}/file2 ) {uri}/file3"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    for opt in ["-c", "--concurrent"] {
        // force nonconcurrent downloads
        cmd("pk pkg fetch")
            .args([opt, "1"])
            .arg(&repo)
            .assert()
            .stdout("")
            .stderr("")
            .success();
    }
}

#[tokio::test]
async fn force() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test"))
        .mount(&server)
        .await;

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file").unwrap();
    assert_eq!(&data, "test");
    let mut prev_modified = fs::metadata("file").unwrap().modified().unwrap();

    // re-run skips downloaded file
    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let mut modified = fs::metadata("file").unwrap().modified().unwrap();
    assert_eq!(modified, prev_modified);

    // -f/--force causes download
    for opt in ["-f", "--force"] {
        cmd("pk pkg fetch")
            .arg(opt)
            .arg(&repo)
            .assert()
            .stdout("")
            .stderr("")
            .success();
        modified = fs::metadata("file").unwrap().modified().unwrap();
        assert_ne!(modified, prev_modified);
        prev_modified = modified;
        let data = fs::read_to_string("file").unwrap();
        assert_eq!(&data, "test");
    }
}

#[tokio::test]
async fn pretend() {
    let server = MockServer::start().await;
    let uri = server.uri();

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file1 {uri}/file2 -> ${{P}}-file2"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    for opt in ["-p", "--pretend"] {
        cmd("pk pkg fetch")
            .arg(opt)
            .arg(&repo)
            .assert()
            .stdout(indoc::formatdoc!(
                "
                {uri}/file1
                {uri}/file2 -> pkg-1-file2
            "
            ))
            .stderr("")
            .success();
    }
}

#[tokio::test]
async fn timeout() {
    let server = MockServer::start().await;
    let delay = Duration::from_secs(1);
    let uri = server.uri();
    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"test")
                .set_delay(delay),
        )
        .mount(&server)
        .await;

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with slow URI connection"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    for opt in ["-t", "--timeout"] {
        cmd("pk pkg fetch")
            .args([opt, "0.1"])
            .arg(&repo)
            .assert()
            .stdout("")
            .stderr(contains(format!("fetch failed: {uri}/file: request timed out")))
            .failure()
            .code(1);
    }
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

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file1"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file2"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-2", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    // version scope
    cmd("pk pkg fetch")
        .arg(repo.path().join("cat/pkg/pkg-1.ebuild"))
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file1").unwrap();
    assert_eq!(&data, "test1");

    // package scope
    cmd("pk pkg fetch")
        .arg(repo.path().join("cat/pkg"))
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file2").unwrap();
    assert_eq!(&data, "test2");
}

#[tokio::test]
async fn rename() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(query_param("p", "pkgcraft"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test"))
        .mount(&server)
        .await;

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI and rename"
        SRC_URI="{uri}/?p=pkgcraft -> pkgcraft-${{PV}}"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    env::set_current_dir(repo.path()).unwrap();

    cmd("pk pkg fetch").assert().stdout("").stderr("").success();
    let data = fs::read_to_string("pkgcraft-1").unwrap();
    assert_eq!(&data, "test");
}

#[tokio::test]
async fn resume() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test resume"))
        .mount(&server)
        .await;

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    // create a partially downloaded file
    let partial_file = dir.path().join("file.part");
    fs::write(&partial_file, "test").unwrap();

    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file").unwrap();
    assert_eq!(&data, "test resume");
    assert!(!partial_file.exists());
}

#[tokio::test]
async fn custom_mirror() {
    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let server = MockServer::start().await;
    let uri = server.uri();
    let name = "mocked";

    Mock::given(method("GET"))
        .and(path("/file1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test1"))
        .mount(&server)
        .await;

    // file without subdirectory
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild using custom mirror"
        SRC_URI="mirror://{name}/file1"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    // unknown mirrors cause failures
    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr(contains("invalid fetchable: mirror unknown: mirror://mocked/file1"))
        .failure()
        .code(1);

    // register mocked mirror
    fs::create_dir_all(repo.path().join("profiles")).unwrap();
    fs::write(
        repo.path().join("profiles/thirdpartymirrors"),
        format!("{name} {uri}/invalid1 {uri}/invalid2 {uri}"),
    )
    .unwrap();

    // iterate through mirrors until download succeeds
    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file1").unwrap();
    assert_eq!(&data, "test1");

    // file with subdirectory
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild using custom mirror with subdirectory"
        SRC_URI="mirror://{name}/path/to/file2"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    // mirror with subdirectory
    fs::write(
        repo.path().join("profiles/thirdpartymirrors"),
        format!("{name} {uri}/invalid1 {uri}/invalid2 {uri}/mirror-dir/"),
    )
    .unwrap();

    // mirrored file combining SRC_URI subdirectory with mirror subdirectory
    Mock::given(method("GET"))
        .and(path("/mirror-dir/path/to/file2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test2"))
        .mount(&server)
        .await;

    // iterate through mirrors until download succeeds
    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file2").unwrap();
    assert_eq!(&data, "test2");
}

#[tokio::test]
async fn redirect() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(ResponseTemplate::new(301).insert_header("Location", "file1"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/file1"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "file2"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/file2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test redirect"))
        .mount(&server)
        .await;

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file").unwrap();
    assert_eq!(&data, "test redirect");
}

#[tokio::test]
async fn restrict() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file"))
        .mount(&server)
        .await;

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with restricted file"
        SRC_URI="file"
        SLOT=0
        RESTRICT="fetch"
    "#};
    repo.create_ebuild_from_str("restricted/file-1", &data)
        .unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with restricted fetchable"
        SRC_URI="{uri}/file"
        SLOT=0
        RESTRICT="fetch"
    "#};
    repo.create_ebuild_from_str("restricted/fetchable-1", &data)
        .unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    // restricted targets are skipped by default
    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();

    // but will be shown as warnings
    cmd("pk pkg fetch -v")
        .arg(repo.path().join("restricted/file"))
        .assert()
        .stdout("")
        .stderr(contains("skipping restricted file: file"))
        .success();
    cmd("pk pkg fetch -v")
        .arg(repo.path().join("restricted/fetchable"))
        .assert()
        .stdout("")
        .stderr(contains(format!("skipping restricted fetchable: {uri}/file")))
        .success();
    assert!(fs::read_to_string("file").is_err());

    // restricted fetchables can be forcibly processed via --restrict
    cmd("pk pkg fetch --restrict")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file").unwrap();
    assert_eq!(&data, "file");
}

#[tokio::test]
async fn selective_restrict() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file1"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/file2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file2"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/file3"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file3"))
        .mount(&server)
        .await;

    let mut repo = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with selective restrictions"
        SRC_URI="{uri}/file1 fetch+{uri}/file2 mirror+{uri}/file3"
        SLOT=0
        RESTRICT="fetch mirror"
    "#};
    repo.create_ebuild_from_str("cat/pkg-1", &data).unwrap();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    cmd("pk pkg fetch")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    assert!(fs::read_to_string("file1").is_err());
    let data = fs::read_to_string("file2").unwrap();
    assert_eq!(&data, "file2");
    let data = fs::read_to_string("file3").unwrap();
    assert_eq!(&data, "file3");

    // restricted fetchables can be forcibly processed via --restrict
    cmd("pk pkg fetch --restrict")
        .arg(&repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("file1").unwrap();
    assert_eq!(&data, "file1");
}
