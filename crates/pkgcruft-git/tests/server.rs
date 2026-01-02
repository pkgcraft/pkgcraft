use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcraft::test::assert_err_re;
use pkgcruft_git::service::PkgcruftServiceBuilder;
use predicates::str::contains;
use tempfile::NamedTempFile;

use crate::cmd;
use crate::git::GitRepo;

#[tokio::test]
async fn socket_errors() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    GitRepo::init(&repo).unwrap();

    // invalid socket
    cmd("pkgcruft-gitd")
        .args(["-b", "invalid-socket"])
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr("pkgcruft-gitd: error: invalid socket: invalid-socket\n")
        .failure()
        .code(1);

    // uds socket already used
    let tmp = NamedTempFile::new().unwrap();
    let socket = tmp.path().to_str().unwrap();
    PkgcruftServiceBuilder::new(repo.path())
        .socket(socket)
        .build()
        .unwrap()
        .spawn()
        .await
        .unwrap();
    cmd("pkgcruft-gitd")
        .args(["-b", socket])
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr(format!("pkgcruft-gitd: error: service already running: {socket}\n"))
        .failure()
        .code(1);

    // uds socket insufficient path permissions
    let result = PkgcruftServiceBuilder::new(repo.path())
        .socket("/path/to/nonexistent/socket")
        .build()
        .unwrap()
        .spawn()
        .await;
    assert_err_re!(result, "failed creating socket dir: /path/to/nonexistent: .*");

    // tcp socket already used
    let service = PkgcruftServiceBuilder::new(repo.path())
        .socket("127.0.0.1:0")
        .build()
        .unwrap()
        .spawn()
        .await
        .unwrap();
    let socket = &service.socket;
    cmd("pkgcruft-gitd")
        .args(["-b", socket])
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr(contains(format!("pkgcruft-gitd: error: failed binding to socket: {socket}")))
        .failure()
        .code(1);
}

#[tokio::test]
async fn start() {
    let repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    GitRepo::init(&repo).unwrap();

    // create temp file for socket location
    let tmp = NamedTempFile::new().unwrap();
    let socket = tmp.path().to_str().unwrap();

    // custom socket
    let service = PkgcruftServiceBuilder::new(repo.path())
        .socket(socket)
        .build()
        .unwrap();
    tokio::spawn(async move { service.start().await });
}
