use std::env;
use std::process::Stdio;
use std::str;

use assert_cmd::Command as assert_command;
use pkgcraft::utils::ARCANIST_RE;
use tempfile::Builder;
use tokio::{io::AsyncBufReadExt, io::BufReader, process::Command};

#[tokio::test]
async fn test_uds() {
    // don't read system/user configs
    env::set_var("ARCANIST_SKIP_CONFIG", "true");
    env::set_var("PAKT_SKIP_CONFIG", "true");

    let tmp_dir = Builder::new().prefix("arcanist.").tempdir().unwrap();
    let socket_path = tmp_dir.path().to_owned().join("arcanist.sock");
    let socket = socket_path.to_str().unwrap();

    let mut arcanist = Command::new(env!("CARGO_BIN_EXE_arcanist"))
        .args(&["--bind", socket])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("arcanist failed to start");

    // wait for arcanist to report it's running
    let stderr = arcanist.stderr.take().expect("no stderr");
    let f = BufReader::new(stderr);
    f.lines().next_line().await.unwrap().unwrap();

    let mut cmd = assert_command::cargo_bin("pakt").unwrap();
    let output = cmd.arg("-c").arg(socket).arg("version").output().unwrap();
    let expected = format!(
        "client: pakt-{0}, server: arcanist-{0}",
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);

    arcanist.kill().await.unwrap();
}

#[tokio::test]
async fn test_tcp() {
    // don't read system/user configs
    env::set_var("ARCANIST_SKIP_CONFIG", "true");
    env::set_var("PAKT_SKIP_CONFIG", "true");

    for addr in ["127.0.0.1:0", "[::]:0"] {
        let mut arcanist = Command::new(env!("CARGO_BIN_EXE_arcanist"))
            .args(&["--bind", addr])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("arcanist failed to start");

        // determine the socket arcanist is running on
        let stderr = arcanist.stderr.take().expect("no stderr");
        let f = BufReader::new(stderr);
        let msg = f.lines().next_line().await.unwrap().unwrap();
        let m = ARCANIST_RE.captures(&msg).unwrap();
        let socket = m.name("socket").unwrap().as_str();
        let url = format!("http://{}", &socket);

        let expected = format!(
            "client: pakt-{0}, server: arcanist-{0}",
            env!("CARGO_PKG_VERSION")
        );
        // verify raw socket arg works
        let mut cmd = assert_command::cargo_bin("pakt").unwrap();
        let output1 = cmd.arg("-c").arg(socket).arg("version").output().unwrap();
        assert_eq!(str::from_utf8(&output1.stdout).unwrap().trim(), expected);
        // as well as regular url
        let mut cmd = assert_command::cargo_bin("pakt").unwrap();
        let output2 = cmd.arg("-c").arg(url).arg("version").output().unwrap();
        assert_eq!(str::from_utf8(&output2.stdout).unwrap().trim(), expected);

        arcanist.kill().await.unwrap();
    }
}
