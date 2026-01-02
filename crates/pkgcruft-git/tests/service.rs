use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcruft_git::service::PkgcruftServiceBuilder;
use tempfile::NamedTempFile;
use tokio::process::Command;

use crate::git::GitRepo;

#[tokio::test]
async fn uds() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    GitRepo::init(&repo).unwrap();
    let tmp = NamedTempFile::new().unwrap();
    let socket = tmp.path().to_str().unwrap();

    // try connecting specific and default socket path
    for socket in [Some(socket), None] {
        let mut service = PkgcruftServiceBuilder::new(repo.path());
        if let Some(value) = socket {
            service = service.socket(value);
        }

        service.build().unwrap().spawn().await.unwrap();
        let ver = env!("CARGO_PKG_VERSION");
        let expected = format!("client: {ver}, server: {ver}\n");

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_pkgcruft-git"));
        if let Some(value) = socket {
            cmd.args(["-c", value]);
        }
        cmd.arg("version");
        let output = cmd.output().await.unwrap();
        let data = String::from_utf8(output.stdout).unwrap();
        assert_eq!(data, expected);
    }
}

#[tokio::test]
async fn tcp() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    GitRepo::init(&repo).unwrap();

    for addr in ["127.0.0.1:0", "[::]:0"] {
        let service = PkgcruftServiceBuilder::new(repo.path())
            .socket(addr)
            .build()
            .unwrap()
            .spawn()
            .await
            .unwrap();
        let url = format!("http://{}", &service.socket);

        let ver = env!("CARGO_PKG_VERSION");
        let expected = format!("client: {ver}, server: {ver}\n");

        // verify both raw socket and url args work
        for serve_addr in [&service.socket, &url] {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_pkgcruft-git"));
            cmd.arg("-c");
            cmd.arg(serve_addr);
            cmd.arg("version");
            let output = cmd.output().await.unwrap();
            let data = String::from_utf8(output.stdout).unwrap();
            assert_eq!(data, expected);
        }
    }
}

#[tokio::test]
async fn scan() {
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    git2::Repository::init(repo.path()).unwrap();

    let tmp = NamedTempFile::new().unwrap();
    let socket = tmp.path().to_str().unwrap();
    let _service = PkgcruftServiceBuilder::new(repo.path())
        .socket(socket)
        .build()
        .unwrap()
        .spawn()
        .await
        .unwrap();

    // empty repo
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pkgcruft-git"));
    cmd.args(["-c", socket]);
    cmd.arg("scan");
    let output = cmd.output().await.unwrap();
    let data = String::from_utf8(output.stdout).unwrap();
    assert_eq!(data, "");

    // invalid pkg
    repo.create_ebuild("cat/pkg-1", &["EAPI=invalid"]).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pkgcruft-git"));
    cmd.args(["-c", socket]);
    cmd.arg("scan");
    let output = cmd.output().await.unwrap();
    let data = String::from_utf8(output.stdout).unwrap();
    let expected = indoc::indoc! {"
        cat/pkg
          MetadataError: version 1: unsupported EAPI: invalid
    "};
    assert_eq!(data, expected);
}
