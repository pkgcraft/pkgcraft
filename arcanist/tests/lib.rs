use std::process::Command;
use std::str;

use assert_cmd::Command as assert_command;
use tempfile::Builder;

#[test]
fn test_version() {
    let tmp_dir = Builder::new().prefix("arcanist.").tempdir().unwrap();
    let socket = tmp_dir.path().to_owned().join("arcanist.sock");
    let socket = socket.to_str().unwrap();
    let mut arcanist = Command::new(env!("CARGO_BIN_EXE_arcanist"))
        .args(&["--bind", socket])
        .spawn()
        .expect("arcanist failed to start");

    let mut cmd = assert_command::cargo_bin("pakt").unwrap();
    let output = cmd.arg("-c").arg(socket).arg("version").output().unwrap();
    let expected = format!(
        "client: pakt-{0}, server: arcanist-{0}",
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);

    arcanist.kill().unwrap();
}
