use std::fs;
use std::os::unix::fs::PermissionsExt;

use itertools::Itertools;
use pkgcraft::repo::ebuild::EbuildRepoBuilder;
use pkgcruft_git::service::PkgcruftServiceBuilder;
use predicates::str::contains;
use tempfile::TempDir;
use tokio::process::Command;

use crate::git::{GitRepo, git};

#[test]
fn invalid_uri() {
    assert_cmd::Command::new(env!("CARGO_BIN_EXE_pkgcruft-git"))
        .args(["-c", "invalid-uri", "push"])
        .assert()
        .stdout("")
        .stderr(contains("pkgcruft-git: error: failed connecting to service: invalid-uri"))
        .failure()
        .code(1);
}

#[tokio::test]
async fn hook() {
    // create bare remote repo
    let remote_dir = TempDir::with_prefix("pkgcruft-git-remote-repo-").unwrap();
    let remote_path = remote_dir.path().to_str().unwrap();
    let remote_repo = GitRepo::init_bare(remote_path).unwrap();

    // create client repo
    let mut repo = EbuildRepoBuilder::new().name("repo").build().unwrap();
    let licenses_dir = repo.path().join("licenses");
    fs::create_dir(&licenses_dir).unwrap();
    fs::write(licenses_dir.join("abc"), "stub license").unwrap();
    let data = indoc::indoc! {r#"
        EAPI=8

        DESCRIPTION="committed package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("a/b-1", data).unwrap();

    // initialize git repo
    let client_repo = GitRepo::init(&repo).unwrap();
    let oid = client_repo.stage(&["*"]).unwrap();
    client_repo.commit(oid, "initial import").unwrap();

    // add remote and push
    git!("remote add origin")
        .current_dir(&repo)
        .arg(remote_path)
        .assert()
        .success();
    git!("push -u origin main")
        .current_dir(&repo)
        .assert()
        .success();

    // create server repo
    let server_dir = TempDir::with_prefix("pkgcruft-git-server-repo-").unwrap();
    let server_path = server_dir.path().to_str().unwrap();
    git!("clone")
        .args([remote_path, server_path])
        .assert()
        .success();
    let _server_repo = GitRepo::init(server_path).unwrap();

    // start pkgcruft-gitd service on server repo
    let service = PkgcruftServiceBuilder::new(server_path)
        .socket("127.0.0.1:0")
        .build()
        .unwrap()
        .spawn()
        .await
        .unwrap();
    let service_uri = &service.socket;

    // verify service is working
    let ver = env!("CARGO_PKG_VERSION");
    let expected = format!("client: {ver}, server: {ver}\n");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pkgcruft-git"));
    cmd.arg("-c");
    cmd.arg(service_uri);
    cmd.arg("version");
    let output = cmd.output().await.unwrap();
    let data = String::from_utf8(output.stdout).unwrap();
    assert_eq!(data, expected);

    // inject hook into remote repo
    let pkgcruft_git = env!("CARGO_BIN_EXE_pkgcruft-git");
    let data = indoc::formatdoc! {r#"
        #!/bin/sh
        {pkgcruft_git} -c {service_uri} push
    "#};
    let hook_path = remote_repo.path().join("hooks/pre-receive");
    fs::write(&hook_path, data).unwrap();
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();

    // create good eclass
    let data = indoc::indoc! {r#"
        # stub eclass
        DEPEND="a/b"
    "#};
    repo.create_eclass("e1", data).unwrap();
    // create package
    let data = indoc::indoc! {r#"
        EAPI=8

        inherit e1

        DESCRIPTION="committed package"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("good/pkg-1", data).unwrap();

    // add commit to client repo
    client_repo.stage(&["*"]).unwrap();
    git!("commit -m good").current_dir(&repo).assert().success();

    // trigger hook via `git push`
    let mut cmd = Command::new("git");
    cmd.arg("push");
    cmd.current_dir(&repo);
    let output = cmd.output().await.unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.is_empty());
    assert_eq!(output.status.code().unwrap(), 0);

    // create bad package
    let data = indoc::indoc! {r#"
        DESCRIPTION="package with unsupported EAPI"
        HOMEPAGE="https://pkgcraft.pkgcraft"
        LICENSE="abc"
        SLOT=0
    "#};
    repo.create_ebuild_from_str("bad/pkg-1", data).unwrap();

    // add commit to client repo
    client_repo.stage(&["*"]).unwrap();
    git!("commit -m bad-pkg")
        .current_dir(&repo)
        .assert()
        .success();

    // trigger hook via `git push`
    let mut cmd = Command::new("git");
    cmd.arg("push");
    cmd.current_dir(&repo);
    let output = cmd.output().await.unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.is_empty());
    let expected = indoc::indoc! {"
        remote: bad/pkg
        remote:   MetadataError: version 1: unsupported EAPI: 0
        remote: pkgcruft-git: error: scanning errors found
        remote:
    "};
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stderr = stderr.lines().map(|x| x.trim().to_string()).join("\n");
    assert!(stderr.contains(expected), "stderr missing expected output:\n{stderr}");
    assert_eq!(output.status.code().unwrap(), 1);

    // create bad eclass
    let data = indoc::indoc! {r#"
        # stub eclass
        cd path
    "#};
    repo.create_eclass("e1", data).unwrap();

    // add commit to client repo
    client_repo.stage(&["*"]).unwrap();
    git!("commit -m bad-eclass")
        .current_dir(&repo)
        .assert()
        .success();

    // trigger hook via `git push`
    let mut cmd = Command::new("git");
    cmd.arg("push");
    cmd.current_dir(&repo);
    let output = cmd.output().await.unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.is_empty());
    let expected = indoc::indoc! {"
        remote: bad/pkg
        remote:   MetadataError: version 1: unsupported EAPI: 0
        remote:
        remote: good/pkg
        remote:   MetadataError: version 1: line 3: inherit: error: failed loading eclass: e1: line 2: disabled builtin: cd
        remote: pkgcruft-git: error: scanning errors found
        remote:
    "};
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stderr = stderr.lines().map(|x| x.trim().to_string()).join("\n");
    assert!(stderr.contains(expected), "stderr missing expected output:\n{stderr}");
    assert_eq!(output.status.code().unwrap(), 1);
}
