use std::env;
use std::process::Command;
use std::str;

use assert_cmd::Command as assert_command;
use pkgcraft::utils::wait_until_file_created;
use tempfile::Builder;

#[test]
fn test_version() {
    // don't read system/user configs
    env::set_var("ARCANIST_SKIP_CONFIG", "true");
    env::set_var("PAKT_SKIP_CONFIG", "true");

    let tmp_dir = Builder::new().prefix("arcanist.").tempdir().unwrap();
    let socket_path = tmp_dir.path().to_owned().join("arcanist.sock");
    let socket = socket_path.to_str().unwrap();
    let mut arcanist = Command::new(env!("CARGO_BIN_EXE_arcanist"))
        .args(&["--bind", socket])
        .spawn()
        .expect("arcanist failed to start");

    // wait for arcanist to bind to its socket file
    wait_until_file_created(&socket_path, Some(5)).unwrap();

    let mut cmd = assert_command::cargo_bin("pakt").unwrap();
    let output = cmd.arg("-c").arg(socket).arg("version").output().unwrap();
    let expected = format!(
        "client: pakt-{0}, server: arcanist-{0}",
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);

    arcanist.kill().unwrap();
}
