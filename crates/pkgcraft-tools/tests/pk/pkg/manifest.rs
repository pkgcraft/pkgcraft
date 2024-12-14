use std::time::Duration;
use std::{env, fs};

use pkgcraft::config::Config;
use pkgcraft::pkg::ebuild::manifest::HashType;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::{cmd, test_data};
use predicates::prelude::*;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn invalid_cwd_target() {
    cmd("pk pkg manifest")
        .assert()
        .stdout("")
        .stderr(contains("invalid ebuild repo"))
        .failure();
}

#[test]
fn nonexistent_path_target() {
    let repo = "path/to/nonexistent/repo";
    cmd(format!("pk pkg manifest {repo}"))
        .assert()
        .stdout("")
        .stderr(contains(format!("invalid path target: {repo}: No such file or directory")))
        .failure();
}

#[test]
fn empty_repo() {
    let data = test_data();
    let repo = data.ebuild_repo("empty").unwrap();
    cmd("pk pkg manifest")
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
    cmd("pk pkg manifest")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(predicate::str::is_empty().not())
        .failure()
        .code(1);
}

#[tokio::test]
async fn unsupported() {
    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with unsupported URI"
        SRC_URI="ftp://pkgcraft.pkgcraft/file"
        SLOT=0
    "#};
    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
    let repo = temp.path();

    cmd("pk pkg manifest")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(contains("fetch failed: ftp://pkgcraft.pkgcraft/file: unsupported URI"))
        .failure()
        .code(1);
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

    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with slow URI connection"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
    let repo = temp.path();

    // TODO: check for timeout error message
    for opt in ["-t", "--timeout"] {
        cmd("pk pkg manifest")
            .args([opt, "0.1"])
            .arg(repo)
            .assert()
            .stdout("")
            .stderr(contains(format!("fetch failed: {uri}/file: request timed out")))
            .failure()
            .code(1);
    }
}

#[tokio::test]
async fn current_dir() {
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

    let mut config = Config::default();
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
    let repo = config
        .add_repo(&temp, false)
        .unwrap()
        .into_ebuild()
        .unwrap();

    env::set_current_dir(temp.path().join("cat/pkg")).unwrap();
    assert!(fs::read_to_string("Manifest").is_err());

    cmd("pk pkg manifest")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string("Manifest").unwrap();
    let expected = indoc::indoc! {"
        DIST file1 5 BLAKE2B c689bf21986252dab8c946042cd73c44995a205da7b8c0816c56ee33894acbace61f27ed94d9ffc2a0d3bee7539565aca834b220af95cc5abb2ceb90946606fe SHA512 b16ed7d24b3ecbd4164dcdad374e08c0ab7518aa07f9d3683f34c2b3c67a15830268cb4a56c1ff6f54c8e54a795f5b87c08668b51f82d0093f7baee7d2981181
        DIST file2 5 BLAKE2B e1b1bfe59054380ac6eb014388b2db3a03d054770ededd9ee148c8b29aa272bbd079344bb40a92d0a754cd925f4beb48c9fd66a0e90b0d341b6fe3bbb4893246 SHA512 6d201beeefb589b08ef0672dac82353d0cbd9ad99e1642c83a1601f3d647bcca003257b5e8f31bdc1d73fbec84fb085c79d6e2677b7ff927e823a54e789140d9
    "};
    assert_eq!(&data, expected);
    let mut prev_modified = fs::metadata("Manifest").unwrap().modified().unwrap();

    // re-run doesn't change file
    cmd("pk pkg manifest")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let mut modified = fs::metadata("Manifest").unwrap().modified().unwrap();
    assert_eq!(modified, prev_modified);

    // -f/--force option cause updates
    for opt in ["-f", "--force"] {
        cmd("pk pkg manifest")
            .arg(opt)
            .assert()
            .stdout("")
            .stderr("")
            .success();
        modified = fs::metadata("Manifest").unwrap().modified().unwrap();
        assert_ne!(modified, prev_modified);
        prev_modified = modified;
        let data = fs::read_to_string("Manifest").unwrap();
        assert_eq!(&data, expected);
    }

    // altering repo manifest-hashes setting changes the content
    let mut config = repo.metadata().config.clone();
    config.manifest_hashes = [HashType::Blake3].into_iter().collect();
    config.manifest_required_hashes = [HashType::Blake3].into_iter().collect();
    config.write().unwrap();

    cmd("pk pkg manifest")
        .assert()
        .stdout("")
        .stderr("")
        .success();
    modified = fs::metadata("Manifest").unwrap().modified().unwrap();
    assert_ne!(modified, prev_modified);
    let data = fs::read_to_string("Manifest").unwrap();
    let expected = indoc::indoc! {"
        DIST file1 5 BLAKE3 3599edef28afa67b9bec983d57416d9a2cc33a166527c3f6ce2aabef96f66c52
        DIST file2 5 BLAKE3 74704b4c3477ac155c2ca3ebbeb8f10db2badac161e331d006af5820f0acca7a
    "};
    assert_eq!(&data, expected);
}

#[tokio::test]
async fn resumed() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test resume"))
        .mount(&server)
        .await;

    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
    let repo = temp.path();

    let dir = tempdir().unwrap();
    env::set_current_dir(&dir).unwrap();

    // create a partially downloaded file
    let partial_file = dir.path().join("file.part");
    fs::write(&partial_file, "test").unwrap();

    cmd("pk pkg manifest")
        .args(["-d", dir.path().to_str().unwrap()])
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    // verify file content
    let data = fs::read_to_string("file").unwrap();
    assert_eq!(&data, "test resume");
    assert!(!partial_file.exists());
    // verify manifest content
    let path = repo.join("cat/pkg/Manifest");
    let data = fs::read_to_string(&path).unwrap();
    let expected = indoc::indoc! {"
        DIST file 11 BLAKE2B 1ca3b378d699a0106a2b3ff84f9daec7596e484e205494c6c81c643b91dadc85c3ddca3fc0f77c16b03922fbb9b38fd11cea1b046b3dc5621af1a5cf054bc1fa SHA512 bca6bd2bb722d500e9e5d9c570a7e382d17e978f4dae51ca689915333f9e8fc4d193dcbcc1adc4c26c010eb1e14ba7f518a8e01f02a4c5f0c75cdab994874c69
    "};
    assert_eq!(&data, expected);
}

#[tokio::test]
async fn stdout() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test"))
        .mount(&server)
        .await;

    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
    let repo = temp.path();

    let expected = indoc::indoc! {"
        DIST file 4 BLAKE2B a71079d42853dea26e453004338670a53814b78137ffbed07603a41d76a483aa9bc33b582f77d30a65e6f29a896c0411f38312e1d66e0bf16386c86a89bea572 SHA512 ee26b0dd4af7e749aa1a8ee3c10ae9923f618980772e473f8819a5d4940e0db27ac185f8a0e1d5f84f88bc887fd67b143732c304cc5fa9ad8e6f57f50028a8ff
    "};

    cmd("pk pkg manifest --stdout")
        .arg(repo)
        .assert()
        .stdout(expected)
        .stderr("")
        .success();
}

#[tokio::test]
async fn invalid_manifest() {
    let server = MockServer::start().await;
    let uri = server.uri();

    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test"))
        .mount(&server)
        .await;

    let mut temp = EbuildRepoBuilder::new().build().unwrap();
    let data = indoc::formatdoc! {r#"
        EAPI=8
        DESCRIPTION="ebuild with mocked SRC_URI"
        SRC_URI="{uri}/file"
        SLOT=0
    "#};
    temp.create_ebuild_from_str("cat/pkg-1", &data).unwrap();
    let repo = temp.path();

    let expected = indoc::indoc! {"
        DIST file 4 BLAKE2B a71079d42853dea26e453004338670a53814b78137ffbed07603a41d76a483aa9bc33b582f77d30a65e6f29a896c0411f38312e1d66e0bf16386c86a89bea572 SHA512 ee26b0dd4af7e749aa1a8ee3c10ae9923f618980772e473f8819a5d4940e0db27ac185f8a0e1d5f84f88bc887fd67b143732c304cc5fa9ad8e6f57f50028a8ff
    "};

    // invalid hash data
    let path = repo.join("cat/pkg/Manifest");
    fs::write(&path, "DIST file 4 BLAKE2B invalid\n").unwrap();
    cmd("pk pkg manifest")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(contains("invalid BLAKE2B hash: invalid"))
        .success();
    let data = fs::read_to_string(&path).unwrap();
    assert_eq!(&data, expected);

    // unsupported hash type
    fs::write(
        &path,
        "DIST file 4 SHA256 84a7775fe0a90c0f649eb18b10779b84626ad8c58dea4a8f24cca83690dd47d4\n",
    )
    .unwrap();
    cmd("pk pkg manifest")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(contains("unsupported hash: SHA256"))
        .success();
    let data = fs::read_to_string(&path).unwrap();
    assert_eq!(&data, expected);

    // missing hash data
    fs::write(&path, "DIST file 4 BLAKE2B\n").unwrap();
    cmd("pk pkg manifest")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr(contains("invalid number of manifest tokens"))
        .success();
    let data = fs::read_to_string(&path).unwrap();
    assert_eq!(&data, expected);

    // hash order doesn't match repo
    fs::write(&path, "DIST file 4 SHA512 ee26b0dd4af7e749aa1a8ee3c10ae9923f618980772e473f8819a5d4940e0db27ac185f8a0e1d5f84f88bc887fd67b143732c304cc5fa9ad8e6f57f50028a8ff BLAKE2B a71079d42853dea26e453004338670a53814b78137ffbed07603a41d76a483aa9bc33b582f77d30a65e6f29a896c0411f38312e1d66e0bf16386c86a89bea572 SHA512 ee26b0dd4af7e749aa1a8ee3c10ae9923f618980772e473f8819a5d4940e0db27ac185f8a0e1d5f84f88bc887fd67b143732c304cc5fa9ad8e6f57f50028a8ff").unwrap();
    cmd("pk pkg manifest")
        .arg(repo)
        .assert()
        .stdout("")
        .stderr("")
        .success();
    let data = fs::read_to_string(&path).unwrap();
    assert_eq!(&data, expected);
}
