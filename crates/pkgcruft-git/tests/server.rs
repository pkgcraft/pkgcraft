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
    let result = PkgcruftServiceBuilder::new(repo.path())
        .socket("invalid-socket")
        .spawn()
        .await;
    assert_err_re!(result, "invalid socket: invalid-socket");

    // uds socket already used
    let tmp = NamedTempFile::new().unwrap();
    let socket = tmp.path().to_str().unwrap();
    PkgcruftServiceBuilder::new(repo.path())
        .socket(socket)
        .spawn()
        .await
        .unwrap();
    cmd("pkgcruft-gitd")
        .args(["-b", socket])
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr(contains(format!("service already running on: {socket}")))
        .failure()
        .code(1);

    // uds socket insufficient path permissions
    let result = PkgcruftServiceBuilder::new(repo.path())
        .socket("/path/to/nonexistent/socket")
        .spawn()
        .await;
    assert_err_re!(result, "failed creating socket dir: /path/to/nonexistent: .*");

    // tcp socket already used
    let service = PkgcruftServiceBuilder::new(repo.path())
        .socket("127.0.0.1:0")
        .spawn()
        .await
        .unwrap();
    let socket = &service.socket;
    cmd("pkgcruft-gitd")
        .args(["-b", socket])
        .arg(repo.path())
        .assert()
        .stdout("")
        .stderr(contains(format!("failed binding to socket: {socket}")))
        .failure()
        .code(1);
}
