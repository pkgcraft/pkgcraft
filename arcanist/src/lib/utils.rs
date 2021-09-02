use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use once_cell::sync::Lazy;
use regex::Regex;
use tokio::{
    io::AsyncBufReadExt,
    io::BufReader,
    process::{Child, Command},
    time::timeout as timeout_future,
};

use crate::error::Error;

pub static ARCANIST_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("^arcanist listening at: (?P<socket>.+)$").unwrap());

pub async fn spawn<S, I, K, V>(
    socket: S,
    env: Option<I>,
    timeout: Option<u64>,
) -> crate::Result<(Child, String)>
where
    S: AsRef<str>,
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    // zero or an unset value effectively means no timeout occurs
    let timeout = match timeout {
        None | Some(0) => Duration::from_secs(u64::MAX),
        Some(x) => Duration::from_secs(x),
    };

    // merge environment settings
    let mut cmd = Command::new("arcanist");
    if let Some(env) = env {
        cmd.env_clear().envs(env);
    }

    // start arcanist detached from the current process while capturing stderr
    let mut arcanist = cmd
        .args(&["--bind", socket.as_ref()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Start(e.to_string()))?;

    // wait for arcanist to report it's running
    let stderr = arcanist.stderr.take().expect("no stderr");
    let f = BufReader::new(stderr);
    let socket = match timeout_future(timeout, f.lines().next_line()).await {
        Ok(Ok(Some(line))) => {
            match ARCANIST_RE.captures(&line) {
                Some(m) => Ok(m.name("socket").unwrap().as_str().to_owned()),
                None => {
                    // try to kill arcanist, but ignore failures
                    arcanist.kill().await.ok();
                    Err(Error::Start(format!(
                        "unknown arcanist message: {:?}",
                        line
                    )))
                }
            }
        }
        Err(_) => {
            arcanist.kill().await.ok();
            Err(Error::Start("timed out".to_string()))
        }
        Ok(Err(e)) => {
            // unknown IO error
            arcanist.kill().await.ok();
            Err(Error::Start(e.to_string()))
        }
        Ok(Ok(None)) => {
            arcanist.kill().await.ok();
            Err(Error::Start("no startup message found".to_string()))
        }
    };

    Ok((arcanist, socket?))
}

pub async fn connect_or_spawn<P: AsRef<Path>>(
    path: P,
    timeout: Option<u64>,
) -> crate::Result<String> {
    let socket_path = path.as_ref();
    let socket = socket_path
        .to_str()
        .ok_or_else(|| Error::Connect(format!("invalid socket path: {:?}", &socket_path)))?
        .to_string();

    if let Err(e) = UnixStream::connect(&socket_path) {
        match e.kind() {
            // spawn arcanist if it's not running
            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound => {
                // remove potentially existing, old socket file
                fs::remove_file(&socket_path).unwrap_or_default();
                // spawn arcanist and wait for it to start
                let env: Option<Vec<(&str, &str)>> = None;
                spawn(&socket, env, timeout).await?;
            }
            _ => return Err(Error::Connect(format!("{}: {:?}", e, &socket_path))),
        }
    }

    Ok(socket)
}
