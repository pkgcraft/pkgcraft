use std::ffi::OsStr;
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use tokio::{
    io::AsyncBufReadExt,
    io::BufReader,
    process::{Child, Command},
    time::timeout as timeout_future,
};

use crate::error::Error;

static PKGCRUFT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^service listening at: (?P<socket>.+)$").unwrap());

pub async fn spawn<S, I, A, O>(
    socket: S,
    env: Option<I>,
    args: Option<A>,
    timeout: Option<u64>,
) -> crate::Result<(Child, String)>
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = (O, O)>,
    A: IntoIterator<Item = O>,
    O: AsRef<OsStr>,
{
    // zero or an unset value effectively means no timeout occurs
    let timeout = match timeout {
        None | Some(0) => Duration::from_secs(u64::MAX),
        Some(x) => Duration::from_secs(x),
    };

    // merge env and args settings
    let mut cmd = Command::new("pkgcruft-gitd");
    if let Some(env) = env {
        cmd.env_clear().envs(env);
    }
    if let Some(args) = args {
        cmd.args(args);
    }

    // start detached from the current process while capturing stderr
    let mut service = cmd
        .arg("--bind")
        .arg(socket.as_ref())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Start(e.to_string()))?;

    // wait to report it's running
    let stderr = service.stderr.take().expect("no stderr");
    let f = BufReader::new(stderr);
    let socket = match timeout_future(timeout, f.lines().next_line()).await {
        Ok(Ok(Some(line))) => {
            match PKGCRUFT_RE.captures(&line) {
                Some(m) => Ok(m.name("socket").unwrap().as_str().to_owned()),
                None => {
                    // try to kill service, but ignore failures
                    service.kill().await.ok();
                    Err(Error::Start(format!("unknown message: {line}")))
                }
            }
        }
        Err(_) => {
            service.kill().await.ok();
            Err(Error::Start("timed out".to_string()))
        }
        Ok(Err(e)) => {
            // unknown IO error
            service.kill().await.ok();
            Err(Error::Start(e.to_string()))
        }
        Ok(Ok(None)) => {
            service.kill().await.ok();
            Err(Error::Start("no startup message found".to_string()))
        }
    };

    Ok((service, socket?))
}
