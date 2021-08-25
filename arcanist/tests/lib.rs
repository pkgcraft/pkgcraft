use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::str;
use std::sync::mpsc;
use std::time::Duration;

use assert_cmd::Command as assert_command;
use notify::{raw_watcher, RawEvent, RecursiveMode, Watcher};
use tempfile::Builder;

fn wait_until_file_created(
    file_path: &PathBuf,
    timeout: Option<u64>,
) -> Result<(), Box<dyn Error>> {
    // zero or an unset value effectively means no timeout occurs
    let timeout = match timeout {
        None | Some(0) => u64::MAX,
        Some(x) => x,
    };

    let (tx, rx) = mpsc::channel();
    let mut watcher = raw_watcher(tx)?;
    // watch parent directory for changes until given file exists
    let file_dir = file_path.parent().unwrap();
    watcher.watch(&file_dir, RecursiveMode::NonRecursive)?;
    if !file_path.exists() {
        loop {
            match rx.recv_timeout(Duration::from_secs(timeout))? {
                RawEvent {
                    path: Some(p),
                    op: Ok(notify::op::CREATE),
                    ..
                } => {
                    if p == *file_path {
                        break;
                    }
                }
                _ => continue,
            }
        }
    }
    watcher.unwatch(file_dir)?;
    Ok(())
}

#[test]
fn test_version() {
    let tmp_dir = Builder::new().prefix("arcanist.").tempdir().unwrap();
    let socket_path = tmp_dir.path().to_owned().join("arcanist.sock");
    let socket = socket_path.to_str().unwrap();
    let mut arcanist = Command::new(env!("CARGO_BIN_EXE_arcanist"))
        .args(&["--bind", socket])
        .spawn()
        .expect("arcanist failed to start");

    // wait for arcanist to bind to its socket file
    wait_until_file_created(&socket_path, None).unwrap();

    let mut cmd = assert_command::cargo_bin("pakt").unwrap();
    let output = cmd.arg("-c").arg(socket).arg("version").output().unwrap();
    let expected = format!(
        "client: pakt-{0}, server: arcanist-{0}",
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);

    arcanist.kill().unwrap();
}
