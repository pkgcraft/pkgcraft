use std::path::PathBuf;
use std::str;
use std::sync::LazyLock;

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::Builder;

static TARGET_DIR: LazyLock<String> = LazyLock::new(|| {
    let tmp_dir = PathBuf::from(env!("CARGO_BIN_EXE_arcanist"));
    tmp_dir.parent().unwrap().to_str().unwrap().to_owned()
});

#[tokio::test]
async fn test_uds() {
    // ignore system/user config and run arcanist from build dir
    let env: [(&str, &str); 1] = [("PATH", &TARGET_DIR)];
    let args = ["--config-none"];

    let tmp_dir = Builder::new().prefix("arcanist.").tempdir().unwrap();
    let socket_path = tmp_dir.path().to_owned().join("arcanist.sock");
    let socket = socket_path.to_str().unwrap();

    let (mut arcanist, socket) = arcanist::spawn(&socket, Some(env), Some(args), Some(5))
        .await
        .unwrap();

    let mut cmd = cargo_bin_cmd!("pakt");
    let output = cmd
        .arg("--config-none")
        .arg("-c")
        .arg(&socket)
        .arg("version")
        .output()
        .unwrap();

    let ver = env!("CARGO_PKG_VERSION");
    let expected = format!("client: pakt-{ver}, server: arcanist-{ver}");
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);

    arcanist.kill().await.unwrap();
}

#[tokio::test]
async fn test_tcp() {
    // ignore system/user config and run arcanist from build dir
    let env: [(&str, &str); 1] = [("PATH", &TARGET_DIR)];
    let args = ["--config-none"];

    for addr in ["127.0.0.1:0", "[::]:0"] {
        let (mut arcanist, socket) = arcanist::spawn(addr, Some(env), Some(args), Some(5))
            .await
            .unwrap();
        let url = format!("http://{}", &socket);

        let ver = env!("CARGO_PKG_VERSION");
        let expected = format!("client: pakt-{ver}, server: arcanist-{ver}");

        // verify both raw socket and url args work
        for serve_addr in [socket, url] {
            let mut cmd = cargo_bin_cmd!("pakt");
            let output = cmd
                .arg("--config-none")
                .arg("-c")
                .arg(&serve_addr)
                .arg("version")
                .output()
                .unwrap();
            assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);
        }

        arcanist.kill().await.unwrap();
    }
}
